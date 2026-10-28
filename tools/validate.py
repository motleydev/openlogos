#!/usr/bin/env python3
"""Offline validator for blueprint data. No network, provisioning or agent execution.
Requires Python 3 and jsonschema. Run from any directory.
"""
from pathlib import Path, PurePosixPath
import copy
import hashlib
import json
import sys
import re
try:
    from jsonschema import Draft202012Validator, ValidationError
    from referencing import Registry, Resource
except ImportError:
    raise SystemExit('Missing jsonschema (with referencing). Use a Python environment with jsonschema >= 4.18; no dependency is downloaded automatically.')
R = Path(__file__).resolve().parents[1]
def load(p):
    path = R / p
    if path.stat().st_size > 2_000_000:
        raise ValueError('Blueprint file exceeds validation size limit: ' + p)
    return json.loads(path.read_text())
def registry():
    # Every schema by its $id, so a schema may $ref another (study-collection -> anchor).
    reg = Registry()
    for path in sorted((R / 'schemas').glob('*.schema.json')):
        sch = load('schemas/' + path.name)
        Draft202012Validator.check_schema(sch)
        reg = reg.with_resource(sch['$id'], Resource.from_contents(sch))
    return reg
REGISTRY = None
def validate(schema_name, data):
    global REGISTRY
    if REGISTRY is None: REGISTRY = registry()
    sch = load('schemas/' + schema_name + '.schema.json')
    Draft202012Validator(sch, registry=REGISTRY).validate(data)
NEGATIVE = 0
def safe_path(value):
    p = PurePosixPath(value)
    if p.is_absolute() or '..' in p.parts or '\\' in value or ':' in value:
        raise ValueError('Unsafe relative path: ' + value)
    if not (R / value).resolve().is_relative_to(R):
        raise ValueError('Path escapes root: ' + value)
def check_dag(tasks):
    ids = {t['id'] for t in tasks}
    if len(ids) != len(tasks):
        raise ValueError('Duplicate task IDs')
    graph = {t['id']: t['depends_on'] for t in tasks}
    done, active = set(), set()
    def visit(i):
        if i not in ids: raise ValueError('Missing dependency: ' + i)
        if i in active: raise ValueError('Task dependency cycle at ' + i)
        if i in done: return
        active.add(i)
        for d in graph[i]: visit(d)
        active.remove(i)
        done.add(i)
    for i in graph: visit(i)
def expect_rejected(fn, label):
    global NEGATIVE
    try: fn()
    except (ValueError, ValidationError, AssertionError): NEGATIVE += 1; return
    raise AssertionError('Negative test was not rejected: ' + label)

def check_ink(snapshot, ink):
    if ink['snapshot_id'] != snapshot['id'] or ink['coordinate_space_id'] != snapshot['coordinate_space_id']:
        raise ValueError('Ink target coordinate mismatch')
    for stroke in ink['strokes']:
        times=[p['elapsed_ms'] for p in stroke['points']]
        if times != sorted(times): raise ValueError('Stroke timestamps out of order')
def check_thread(thread):
    evidence={e['id']:e for e in thread['evidence']}
    messages={m['id'] for m in thread['messages']}
    if len(messages)!=len(thread['messages']): raise ValueError('Duplicate message IDs')
    for e in evidence.values():
        if e['source_id'] not in thread['selected_source_ids']: raise ValueError('Out-of-scope evidence')
    seen=set()
    for m in thread['messages']:
        if m['parent_id'] is not None and m['parent_id'] not in seen: raise ValueError('Missing or noncausal parent')
        if any(e not in evidence for e in m['evidence_ids']): raise ValueError('Fabricated evidence ID')
        seen.add(m['id'])

PROFILE_LIMITS={'compact':1,'regular':3,'wide':6,'ultra-wide':6}
def check_workspace(wp):
    # B062: layout profiles place every pane exactly once and own no link state or anchor
    # (specs/18-platform-capabilities.md, "Workspace layout profiles").
    ids=[p['id'] for p in wp['panes']]
    if len(set(ids))!=len(ids): raise ValueError('Duplicate pane IDs')
    if wp['active_pane_id'] not in ids: raise ValueError('Active pane is not a pane')
    if abs(sum(p['width_fraction'] for p in wp['panes'])-1)>1e-8: raise ValueError('Pane widths do not sum to one')
    groups={p['link_group'] for p in wp['panes'] if p['link_group'] is not None}
    for p in wp['panes']:
        if p['pinned'] and p['link_group'] is not None: raise ValueError('Pinned pane inside a link group: '+p['id'])
    for cls,profile in wp.get('layout_profiles',{}).items():
        if cls not in PROFILE_LIMITS: raise ValueError('Unknown device class: '+cls)
        placed=profile['visible_pane_ids']+profile['tab_order']
        if sorted(placed)!=sorted(ids): raise ValueError('Profile '+cls+' does not place every pane exactly once')
        collapsed=set(profile.get('collapsed_link_groups',[]))
        group_of={p['id']:p['link_group'] for p in wp['panes']}
        columns,seen=0,set()
        for pid in profile['visible_pane_ids']:
            g=group_of[pid]
            if g in collapsed and g in seen: continue
            columns+=1; seen.add(g)
        if columns>PROFILE_LIMITS[cls]: raise ValueError('Profile '+cls+' shows more columns than its class allows')
        widths=profile.get('widths',{})
        if widths and not set(widths)<=set(profile['visible_pane_ids']): raise ValueError('Profile '+cls+' sizes a pane it does not show')
        if widths and abs(sum(widths.values())-1)>1e-8: raise ValueError('Profile '+cls+' widths do not sum to one')
        if not set(profile.get('collapsed_link_groups',[]))<=groups: raise ValueError('Profile '+cls+' collapses a link group no pane belongs to')
def core_api_version():
    text=(R/'crates/core/src/lib.rs').read_text()
    m=re.search(r'pub const API_VERSION: &str = "([^"]+)"', text)
    if not m: raise ValueError('API_VERSION not found in crates/core/src/lib.rs')
    return m.group(1)
def token_text(t):
    return f"{t['edition']}:{t['book']}.{t['chapter']}.{t['verse']}.{t['word']}{t['suffix'] or ''}"
def check_lineage(lineage):
    seen_from, seen_to = set(), set()
    shapes={'same':(1,1,1,1),'split':(1,1,2,None),'merge':(2,None,1,1),'insert':(0,0,1,None),'delete':(1,None,0,0)}
    for i,e in enumerate(lineage['entries']):
        fmin,fmax,tmin,tmax=shapes[e['kind']]
        f,t=len(e['from']),len(e['to'])
        if f<fmin or (fmax is not None and f>fmax) or t<tmin or (tmax is not None and t>tmax):
            raise ValueError(f'Lineage row {i} is {e["kind"]} with {f} old and {t} new ids')
        if e['kind']=='same' and e['from'][0]!=e['to'][0]: raise ValueError(f'Lineage row {i} is same but changes the id')
        for tok in e['from']+e['to']:
            if tok['edition']!=lineage['edition_id']: raise ValueError('Lineage names a token of another edition')
        for tok in e['from']:
            k=token_text(tok)
            if k in seen_from: raise ValueError('Old id in two lineage rows: '+k)
            seen_from.add(k)
        for tok in e['to']:
            k=token_text(tok)
            if k in seen_to: raise ValueError('New id in two lineage rows: '+k)
            seen_to.add(k)
    if lineage['from_revision']==lineage['to_revision']: raise ValueError('Lineage from a revision to itself')
def check_alignment(a):
    for i,r in enumerate(a['records']):
        for tok in r['source_token_ids']:
            if tok['edition']!=a['source_edition']: raise ValueError(f'Alignment record {i}: source token of another edition')
        for tok in r['target_token_ids']:
            if tok['edition']!=a['target_edition']: raise ValueError(f'Alignment record {i}: target token of another edition')
        if r['status']=='null' and (len(r['source_token_ids'])==0)==(len(r['target_token_ids'])==0):
            raise ValueError(f'Alignment record {i}: a null record has exactly one empty side')
    if a['source_tokenization_hash']==a['target_tokenization_hash']: raise ValueError('Two token layers with one hash')
def check_collection(c):
    ids=[i['id'] for i in c['items']]+[g['id'] for g in c['groups']]
    if len(ids)!=len(set(ids)): raise ValueError('Duplicate collection ids')
    groups={g['id'] for g in c['groups']}
    for item in c['items']:
        if item.get('group_id') is not None and item['group_id'] not in groups: raise ValueError('Item under unknown group: '+item['id'])
        src=item.get('source')
        if src:
            loc=src['locator']
            if not (loc.get('reference') or loc.get('work_locator') or loc.get('pdf')): raise ValueError('Source item without a locator: '+item['id'])
            if src.get('anchor') and (src['anchor']['edition_id']!=src['edition_id'] or src['anchor']['content_revision']!=src['content_revision']):
                raise ValueError('Anchor disagrees with provenance: '+item['id'])
        if item['kind']=='source_excerpt' and item.get('text') is not None and item['source']['rights']['quotation']!='allow':
            raise ValueError('Quoted text stored without permission: '+item['id'])
        if item['kind']=='research_excerpt' and not item['generated_label'].strip(): raise ValueError('Machine output without a label: '+item['id'])
def check_receipt(rc, collection):
    if rc['collection_id']!=collection['id']: raise ValueError('Receipt for another collection')
    if rc['writer']['core_api_version']!=core_api_version(): raise ValueError('Receipt example must name the current core API version')
    items={i['id']:i for i in collection['items']}
    seen=set()
    safe_path(rc['resource_folder'])
    for e in rc['entries']:
        safe_path(e['resource_path'])
        if e['item_id'] in seen: raise ValueError('Two receipt entries for one item: '+e['item_id'])
        seen.add(e['item_id'])
        if e['block_id']!='ol-'+e['item_id']: raise ValueError('Block id is not ol-<item id>: '+e['block_id'])
        item=items.get(e['item_id'])
        if item is None: raise ValueError('Receipt entry for an item not in the collection: '+e['item_id'])
        if e['item_revision']>item['revision']: raise ValueError('Receipt entry ahead of its item: '+e['item_id'])
        permitted=item['kind']=='source_excerpt' and item.get('text') is not None and item['source']['rights']['quotation']=='allow'
        if e['disposition']=='quoted' and not permitted: raise ValueError('Quoted block without permission: '+e['item_id'])
        if e['disposition']!='quoted' and not (e.get('reason') or '').strip(): raise ValueError('Non-quoted entry without a reason: '+e['item_id'])
def check_fixtures():
    # Conformance fixtures: id equals file name, kind known, description and license present.
    kinds={'text','versification','references','reanchor','lineage','alignment','workspace_layout'}
    count=0
    for path in sorted((R/'crates/conformance/fixtures').glob('*.json')):
        f=load('crates/conformance/fixtures/'+path.name)
        if f['id']!=path.stem: raise ValueError('Fixture id differs from file name: '+path.name)
        if f['kind'] not in kinds: raise ValueError('Unknown fixture kind: '+f['kind'])
        if not f['description'] or not f['license']: raise ValueError('Fixture without description or license: '+path.name)
        count+=1
    return count

def main():
    pairs = [
        ('agents','specs/agents.json'), ('backlog','specs/backlog.json'),
        ('model-policy','specs/model-policy.json'), ('sources','specs/sources.json'),
        ('claim','examples/claim.json'), ('package-manifest','examples/package-manifest.json'),
        ('rights-record','examples/rights-record.json'),
        ('research-provider-policy','specs/research-provider-policy.json'),
        ('workspace','examples/workspace.json'),
        ('workspace','examples/archive/workspace-1.0.json'),
        ('reading-snapshot','examples/reading-snapshot.json'),
        ('ink-document','examples/ink-document.json'),
        ('research-thread','examples/research-thread.json'),
        ('package-manifest','examples/archive/package-manifest-1.0.json'),
        ('anchor','examples/anchor.json'), ('verse-map','examples/verse-map.json'),
        ('reference-mapping','examples/reference-mapping.json'),
        ('token-lineage','examples/token-lineage.json'), ('alignment','examples/alignment.json'),
        ('study-collection','examples/study-collection.json'),
        ('vault-export-receipt','examples/vault-export-receipt.json')]
    for s,p in pairs: validate(s,load(p))
    agents = load('specs/agents.json')['agents']
    tasks = load('specs/backlog.json')['tasks']
    model = load('specs/model-policy.json')
    roles = {a['external_id'] for a in agents}
    assert len(roles) == len(agents), 'Duplicate agent IDs'
    assert sum(a['budget_weight'] for a in agents) == 100, 'Budget weights must sum to 100'
    tiers = {t['id'] for t in model['model_tiers']}
    for a in agents:
        assert a['default_model_tier'] in tiers
        assert a['initial_state'] == 'paused'
        assert a['external_id'] not in a['reviewer_roles'], 'Self-only review assignment'
        for reviewer in a['reviewer_roles']: assert reviewer in roles
        for p in a['work_scope']: safe_path(p)
    for t in tasks:
        assert t['owner_role'] in roles, 'Missing owner'
        assert t['initial_status'] == ('blocked' if t['depends_on'] else 'ready_for_scaffold')
        for p in t['spec_paths']:
            safe_path(p)
            assert (R/p).is_file(), 'Missing referenced spec: ' + p
    check_dag(tasks)
    shared=load('specs/agents.json')['shared_policy_path']
    safe_path(shared)
    assert (R/shared).is_file()
    source_ids={s['id'] for s in load('specs/sources.json')['sources']}
    assert len(source_ids)==len(load('specs/sources.json')['sources'])
    for p in R.rglob('*.md'):
        for sid in re.findall(r'\bS\d{2}\b',p.read_text()):
            assert sid in source_ids, 'Unknown research citation: ' + sid
    for p in R.rglob('*'):
        assert p.name not in {'AGENTS.md','CLAUDE.md'}, 'Auto-loaded instructions forbidden in this data bundle'
    manifest=load('examples/package-manifest.json')
    for artifact in manifest['artifacts']:
        safe_path(artifact['path'])
        content=(R/'examples'/artifact['path']).read_bytes()
        assert hashlib.sha256(content).hexdigest()==artifact['sha256']
        assert len(content)==artifact['size_bytes']
    safe_path(manifest['attribution_path'])
    assert (R/'examples'/manifest['attribution_path']).is_file()
    claim=load('examples/claim.json')
    assert claim['fixture_only'] and claim['review_state']=='proposed'
    assert load('examples/rights-record.json')['status']=='unknown'
    bad=copy.deepcopy(load('specs/agents.json'))
    bad['agents'][0]['adapter_shell_command']='not executable'
    expect_rejected(lambda:validate('agents',bad),'unknown agent field')
    cyclic=copy.deepcopy(tasks)
    cyclic[0]['depends_on']=[cyclic[0]['id']]
    expect_rejected(lambda:check_dag(cyclic),'cycle')
    missing=copy.deepcopy(tasks)
    missing[0]['depends_on']=['B999']
    expect_rejected(lambda:check_dag(missing),'missing dependency')
    expect_rejected(lambda:safe_path('../outside'),'path traversal')
    unsigned=copy.deepcopy(manifest)
    unsigned['fixture_only']=False
    expect_rejected(lambda:validate('package-manifest',unsigned),'unsigned production manifest')
    wp=load('examples/workspace.json')
    check_workspace(wp)
    assert wp['schema_version']=='1.1' and len(wp['panes'])==5, 'Workspace example is the five-pane two-group workspace of specs/18'
    assert len({p['link_group'] for p in wp['panes']})==2 and {'wide','regular'}<=set(wp['layout_profiles'])
    old_wp=load('examples/archive/workspace-1.0.json')
    check_workspace(old_wp)
    assert old_wp['schema_version']=='1.0' and len(old_wp['panes'])==3 and 'layout_profiles' not in old_wp
    dropped=copy.deepcopy(wp); dropped['layout_profiles']['wide']['visible_pane_ids'].remove('p5')
    expect_rejected(lambda:check_workspace(dropped),'layout profile that drops a pane')
    twice=copy.deepcopy(wp); twice['layout_profiles']['regular']['tab_order'].append('p1')
    expect_rejected(lambda:check_workspace(twice),'layout profile that places a pane twice')
    cross=copy.deepcopy(wp); cross['layout_profiles']['regular']['compact']={'visible_pane_ids':['p2'],'tab_order':['p1','p3','p4','p5'],'notes_dock_open':False}
    expect_rejected(lambda:validate('workspace',cross),'layout profile that carries another class\'s layout')
    foreign_class=copy.deepcopy(wp); foreign_class['layout_profiles']['desktop']=foreign_class['layout_profiles']['wide']
    expect_rejected(lambda:validate('workspace',foreign_class),'layout profile for an unknown device class')
    crowded=copy.deepcopy(wp); crowded['layout_profiles']['compact']={'visible_pane_ids':['p1','p2'],'tab_order':['p3','p4','p5'],'notes_dock_open':False}
    expect_rejected(lambda:check_workspace(crowded),'compact profile with two columns')
    stacked=copy.deepcopy(crowded); stacked['layout_profiles']['compact']['collapsed_link_groups']=['1']
    check_workspace(stacked)  # the same two panes in one collapsed group are one column
    old_with_profiles=copy.deepcopy(old_wp); old_with_profiles['layout_profiles']={}
    expect_rejected(lambda:validate('workspace',old_with_profiles),'1.0 workspace with 1.1 profiles')
    new_without_profiles=copy.deepcopy(wp); del new_without_profiles['layout_profiles']
    expect_rejected(lambda:validate('workspace',new_without_profiles),'1.1 workspace without profiles')
    sn=load('examples/reading-snapshot.json')
    ink=load('examples/ink-document.json')
    check_ink(sn,ink)
    safe_path(sn['background']['path'])
    assert hashlib.sha256((R/'examples'/sn['background']['path']).read_bytes()).hexdigest()==sn['background']['sha256']
    thread=load('examples/research-thread.json')
    check_thread(thread)
    leaked=copy.deepcopy(thread); leaked['api_key']='fixture-forbidden'
    expect_rejected(lambda:validate('research-thread',leaked),'secret in exported thread')
    moved=copy.deepcopy(ink); moved['coordinate_space_id']='wrong'
    expect_rejected(lambda:check_ink(sn,moved),'ink coordinate mismatch')
    invented=copy.deepcopy(thread); invented['messages'][-1]['evidence_ids']=['not-retrieved']
    expect_rejected(lambda:check_thread(invented),'fabricated evidence')
    foreign=copy.deepcopy(thread); foreign['evidence'][0]['source_id']='private-unselected'
    expect_rejected(lambda:check_thread(foreign),'unselected evidence source')
    provider=load('specs/research-provider-policy.json')
    assert provider['automatic_paid_fallback'] is False and provider['credentials_in_exports'] is False
    assert {s for route in provider['routes'] for s in route['source_ids']} <= source_ids
    # B006: identity, anchor and collection contracts.
    assert manifest['schema_version']=='1.1' and manifest['core_api_version']==core_api_version(), 'Manifest example must name the current core API version'
    assert load('examples/archive/package-manifest-1.0.json')['schema_version']=='1.0'
    lineage=load('examples/token-lineage.json'); check_lineage(lineage)
    alignment=load('examples/alignment.json'); check_alignment(alignment)
    collection=load('examples/study-collection.json'); check_collection(collection)
    receipt=load('examples/vault-export-receipt.json'); check_receipt(receipt, collection)
    assert collection['owner_id']=='fixture-device' and receipt['vault_id'].startswith('vault-fixture')
    mapping=load('examples/reference-mapping.json')
    assert mapping['status']!='unmapped' or not mapping['to']
    anchor=load('examples/anchor.json')
    assert anchor['range']['start']<=anchor['range']['end'] and anchor['quote']['exact']=='loved the world'
    fixtures=check_fixtures()
    old_manifest=copy.deepcopy(load('examples/archive/package-manifest-1.0.json')); old_manifest['core_api_version']=core_api_version()
    expect_rejected(lambda:validate('package-manifest',old_manifest),'1.0 manifest with 1.1 fields')
    new_manifest=copy.deepcopy(manifest); del new_manifest['edition_revisions']
    expect_rejected(lambda:validate('package-manifest',new_manifest),'1.1 manifest without edition revisions')
    bad_lineage=copy.deepcopy(lineage); bad_lineage['entries'][0]['to'].append(bad_lineage['entries'][0]['to'][0])
    expect_rejected(lambda:validate('token-lineage',bad_lineage),'same row with two targets (schema)')
    bad_lineage=copy.deepcopy(lineage); bad_lineage['entries'][0]['to'][0]['word']=9
    expect_rejected(lambda:check_lineage(bad_lineage),'same row that changes the id')
    bad_lineage=copy.deepcopy(lineage); bad_lineage['entries'].append(copy.deepcopy(lineage['entries'][0]))
    expect_rejected(lambda:check_lineage(bad_lineage),'old id in two lineage rows')
    bad_alignment=copy.deepcopy(alignment); bad_alignment['records'][2]['target_token_ids']=[alignment['records'][1]['target_token_ids'][0]]
    expect_rejected(lambda:validate('alignment',bad_alignment),'null record with both sides (schema)')
    bad_alignment=copy.deepcopy(alignment); bad_alignment['records'][0]['origin']='eflomal'
    expect_rejected(lambda:validate('alignment',bad_alignment),'origin without kind')
    bad_alignment=copy.deepcopy(alignment); bad_alignment['records'][0]['source_token_ids'][0]['edition']='bsb'
    expect_rejected(lambda:check_alignment(bad_alignment),'source token of the target edition')
    bad_alignment=copy.deepcopy(alignment); bad_alignment['source_tokenization_hash']='ABC'
    expect_rejected(lambda:validate('alignment',bad_alignment),'tokenization hash not sha256 hex')
    bad_collection=copy.deepcopy(collection); bad_collection['items'][2]['text']='copied anyway'
    expect_rejected(lambda:validate('study-collection',bad_collection),'quoted text under denied rights (schema)')
    expect_rejected(lambda:check_collection(bad_collection),'quoted text under denied rights (check)')
    bad_collection=copy.deepcopy(collection); bad_collection['items'][5]['generated_label']=''
    expect_rejected(lambda:validate('study-collection',bad_collection),'machine output without a label')
    bad_collection=copy.deepcopy(collection); bad_collection['items'][1]['source']['locator']={'reference':None,'work_locator':None,'pdf':None}
    expect_rejected(lambda:validate('study-collection',bad_collection),'source without a locator')
    bad_collection=copy.deepcopy(collection); bad_collection['items'][0]['source']['content_revision']='bsb-2099'
    expect_rejected(lambda:check_collection(bad_collection),'anchor disagreeing with provenance')
    bad_collection=copy.deepcopy(collection); bad_collection['items'][4]['id']='Observation 1'
    expect_rejected(lambda:validate('study-collection',bad_collection),'item id not stable-id shaped')
    bad_receipt=copy.deepcopy(receipt); bad_receipt['entries'][0]['resource_path']='../outside.md'
    expect_rejected(lambda:validate('vault-export-receipt',bad_receipt),'receipt path traversal (schema)')
    expect_rejected(lambda:check_receipt(bad_receipt, collection),'receipt path traversal (check)')
    bad_receipt=copy.deepcopy(receipt); bad_receipt['entries'][0]['block_id']='ol-other'
    expect_rejected(lambda:check_receipt(bad_receipt, collection),'block id not derived from item id')
    bad_receipt=copy.deepcopy(receipt); bad_receipt['entries'][1]['disposition']='quoted'
    expect_rejected(lambda:check_receipt(bad_receipt, collection),'quoted block for a reference-only item')
    bad_receipt=copy.deepcopy(receipt); bad_receipt['entries'][2]['reason']=None
    expect_rejected(lambda:validate('vault-export-receipt',bad_receipt),'metadata-only entry without a reason')
    bad_receipt=copy.deepcopy(receipt); bad_receipt['entries'][0]['item_revision']=99
    expect_rejected(lambda:check_receipt(bad_receipt, collection),'receipt entry ahead of its item')
    bad_mapping=copy.deepcopy(mapping); bad_mapping['status']='unmapped'
    expect_rejected(lambda:validate('reference-mapping',bad_mapping),'unmapped mapping with targets')
    bad_map=load('examples/verse-map.json'); bad_map['entries'][5]['to']=bad_map['entries'][0]['to']
    expect_rejected(lambda:validate('verse-map',bad_map),'absent row with a target')
    bad_anchor=copy.deepcopy(anchor); bad_anchor['tokens'][0]['book']='Gen2'
    expect_rejected(lambda:validate('anchor',bad_anchor),'token id with an unknown book')
    print(f'PASS: {len(pairs)} schema/example validations; {len(agents)} agent records; {len(tasks)} acyclic tasks; {fixtures} conformance fixtures; source references, budgets and fixture hashes; {NEGATIVE} negative tests, including secret leakage, ink coordinates, research evidence scope, lineage and alignment shapes, quotation rights, receipt identity and workspace layout profiles.')
    print('Scope: blueprint structure only. No app, Nix build, cryptography, source clearance or Paperclip deployment has been tested.')
if __name__=='__main__': main()

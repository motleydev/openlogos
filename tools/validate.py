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
def device_id_for(signing_public_hex):
    return 'dev-'+hashlib.sha256(bytes.fromhex(signing_public_hex)).hexdigest()[:32]
def check_sync_batch(batch):
    # B021: globally unique op ids, contiguous per-device sequences, payload hashes that match.
    ids=[op['op_id'] for op in batch['operations']]
    if len(ids)!=len(set(ids)): raise ValueError('Duplicate operation id in batch')
    by_device={}
    for op in batch['operations']:
        if op['vault_id']!=batch['vault_id']: raise ValueError('Operation of another vault in batch: '+op['op_id'])
        if op['op_id'] in op['parents']: raise ValueError('Operation lists itself as a parent: '+op['op_id'])
        by_device.setdefault(op['device_id'],[]).append(op['device_sequence'])
        if op['payload_hash'] is not None and batch['payloads'] is not None:
            sealed=batch['payloads'].get(op['op_id'])
            if sealed is None: raise ValueError('Operation commits to a payload the batch lacks: '+op['op_id'])
            digest=hashlib.sha256(bytes.fromhex(sealed['nonce'])+bytes.fromhex(sealed['ciphertext'])).hexdigest()
            if digest!=op['payload_hash']: raise ValueError('Payload hash mismatch: '+op['op_id'])
            if sealed['key_epoch']!=op['key_epoch']: raise ValueError('Payload sealed at another epoch: '+op['op_id'])
    for p in batch['payloads'] or {}:
        if p not in ids: raise ValueError('Payload without its operation: '+p)
    for device,seqs in by_device.items():
        seqs=sorted(seqs)
        if seqs!=list(range(seqs[0],seqs[0]+len(seqs))): raise ValueError(f'Sequence gap or repeat for {device}: {seqs}')
def check_roster(r):
    # B021: device ids bound to signing keys, one founder, approvers enrolled no later, one wrap per epoch.
    ids=[d['device']['device_id'] for d in r['devices']]
    if len(ids)!=len(set(ids)): raise ValueError('Duplicate device in roster')
    records={d['device']['device_id']:d for d in r['devices']}
    if sum(1 for d in r['devices'] if d['approval']['by']=='founder')!=1: raise ValueError('Roster must have exactly one founder')
    for d in r['devices']:
        pub=d['device']
        if pub['device_id']!=device_id_for(pub['signing_public']): raise ValueError('Device id is not the hash of its signing key: '+pub['device_id'])
        if pub['signing_public']==pub['exchange_public']: raise ValueError('Signing and exchange keys must differ: '+pub['device_id'])
        a=d['approval']
        if a['by']=='device':
            approver=records.get(a['device_id'])
            if approver is None: raise ValueError('Approval by a device not in the roster: '+pub['device_id'])
            if approver['enrolled_epoch']>d['enrolled_epoch']: raise ValueError('Approval by a device enrolled later: '+pub['device_id'])
        rev=d['revocation']
        last=r['key_epoch'] if rev is None else rev['epoch_after']-1
        if rev is not None:
            if rev['by'] not in records or rev['by']==pub['device_id']: raise ValueError('Revocation by an unknown device or by itself: '+pub['device_id'])
            if rev['epoch_after']>r['key_epoch']: raise ValueError('Revocation names a future epoch: '+pub['device_id'])
        epochs=[w['key_epoch'] for w in d['wrapped_keys']]
        if epochs!=list(range(d['enrolled_epoch'],last+1)): raise ValueError(f'{pub["device_id"]} holds keys for epochs {epochs}, expected {d["enrolled_epoch"]}..{last}')
        for w in d['wrapped_keys']:
            if w['recipient']['kind']!='device' or w['recipient']['device_id']!=pub['device_id']: raise ValueError('Wrapped key for another recipient: '+pub['device_id'])
    rec=r['recovery']
    if [w['key_epoch'] for w in rec['wrapped_keys']]!=list(range(1,r['key_epoch']+1)): raise ValueError('Recovery must hold every epoch')
    for w in rec['wrapped_keys']:
        if w['recipient']['kind']!='recovery' or w['recipient']['recovery_public']!=rec['recovery_public']: raise ValueError('Recovery wrap for another identity')
    if r['principal'] is not None and r['principal']['linked_by'] not in records: raise ValueError('Principal linked by an unknown device')
def check_recovery_export(e):
    check_roster(e['roster'])
    if e['roster']['vault_id']!=e['vault_id']: raise ValueError('Recovery export carries another vault\'s roster')
    if e['roster']['recovery']['recovery_public']!=e['recovery_public']: raise ValueError('Recovery public key differs from the roster\'s')
    if not (e['key_epoch']==e['roster']['key_epoch']==e['wrapped_vault_key']['key_epoch']): raise ValueError('Recovery export epochs disagree')
    if e['wrapped_vault_key']['recipient']!={'kind':'recovery','recovery_public':e['recovery_public']}: raise ValueError('Recovery export key not wrapped to the recovery identity')
def check_sync_fixture(f):
    ids={op['op_id'] for op in f['operations']}
    check_sync_batch({'vault_id':f['roster']['vault_id'],'operations':f['operations'],'payloads':None})
    check_roster(f['roster'])
    for d in f['deliveries']:
        if set(d['order'])-ids: raise ValueError('Delivery names an operation outside the fixture: '+d['name'])
    for device in f['active_devices']:
        rec=[d for d in f['roster']['devices'] if d['device']['device_id']==device]
        if not rec or rec[0]['revocation'] is not None: raise ValueError('Active device list names a revoked or unknown device')
    if f['expected_state']!=f['expected_state_after_compaction']: raise ValueError('Compaction must not change the merged state')
    if set(f['expected_compacted'])-set(f['expected_state']['deleted']): raise ValueError('Compacted entity without a tombstone')
def check_fixtures():
    # Conformance fixtures: id equals file name, kind known, description and license present.
    kinds={'text','versification','references','reanchor','lineage','alignment','sync-merge'}
    count=0
    for path in sorted((R/'crates/conformance/fixtures').glob('*.json')):
        f=load('crates/conformance/fixtures/'+path.name)
        if f['id']!=path.stem: raise ValueError('Fixture id differs from file name: '+path.name)
        if f['kind'] not in kinds: raise ValueError('Unknown fixture kind: '+f['kind'])
        if not f['description'] or not f['license']: raise ValueError('Fixture without description or license: '+path.name)
        if f['kind']=='sync-merge': check_sync_fixture(f)
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
        ('reading-snapshot','examples/reading-snapshot.json'),
        ('ink-document','examples/ink-document.json'),
        ('research-thread','examples/research-thread.json'),
        ('package-manifest','examples/archive/package-manifest-1.0.json'),
        ('anchor','examples/anchor.json'), ('verse-map','examples/verse-map.json'),
        ('reference-mapping','examples/reference-mapping.json'),
        ('token-lineage','examples/token-lineage.json'), ('alignment','examples/alignment.json'),
        ('study-collection','examples/study-collection.json'),
        ('vault-export-receipt','examples/vault-export-receipt.json'),
        ('sync-operation','examples/sync-operation.json'),
        ('device-enrollment','examples/device-enrollment.json'),
        ('recovery-export','examples/recovery-export.json')]
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
    assert len(wp['panes'])==3 and all(p['kind']=='reading' for p in wp['panes'])
    assert wp['active_pane_id'] in {p['id'] for p in wp['panes']}
    assert abs(sum(p['width_fraction'] for p in wp['panes'])-1)<1e-8
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
    # B021: vault identity and encrypted sync records.
    batch=load('examples/sync-operation.json'); check_sync_batch(batch)
    roster=load('examples/device-enrollment.json'); check_roster(roster)
    export=load('examples/recovery-export.json'); check_recovery_export(export)
    assert roster['key_epoch']==2 and any(d['revocation'] for d in roster['devices']), 'Roster example must show a revocation and rotation'
    assert len(batch['operations'])==8 and any(op['payload_hash'] is None for op in batch['operations'])
    assert all(len(set(op['parents']))==len(op['parents']) for op in batch['operations'])
    sync_fixture=load('crates/conformance/fixtures/sync-operation-log-merge.json')
    assert sync_fixture['roster']==roster and len(sync_fixture['deliveries'])==4
    leaked=copy.deepcopy(roster); leaked['devices'][0]['signing_secret']='00'*32
    expect_rejected(lambda:validate('device-enrollment',leaked),'private key in roster')
    leaked=copy.deepcopy(roster); leaked['devices'][0]['device']['exchange_secret']='00'*32
    expect_rejected(lambda:validate('device-enrollment',leaked),'private exchange key in device record')
    leaked=copy.deepcopy(roster); leaked['vault_key']='00'*32
    expect_rejected(lambda:validate('device-enrollment',leaked),'raw vault key in roster')
    leaked=copy.deepcopy(roster); leaked['recovery']['wrapped_keys'][0]['plaintext_key']='00'*32
    expect_rejected(lambda:validate('device-enrollment',leaked),'plaintext key beside a wrap')
    leaked=copy.deepcopy(batch); leaked['access_token']='bearer fixture-forbidden'
    expect_rejected(lambda:validate('sync-operation',leaked),'bearer token in sync batch')
    leaked=copy.deepcopy(batch); leaked['operations'][0]['body']='plaintext note'
    expect_rejected(lambda:validate('sync-operation',leaked),'plaintext body on an operation header')
    leaked=copy.deepcopy(batch); next(iter(leaked['payloads'].values()))['plaintext']='note'
    expect_rejected(lambda:validate('sync-operation',leaked),'plaintext beside a sealed payload')
    leaked=copy.deepcopy(export); leaked['recovery_secret']='00'*32
    expect_rejected(lambda:validate('recovery-export',leaked),'recovery secret in recovery export')
    leaked=copy.deepcopy(export); leaked['passphrase']='hunter2'
    expect_rejected(lambda:validate('recovery-export',leaked),'passphrase in recovery export')
    leaked=copy.deepcopy(export); leaked['roster']['devices'][0]['wrapped_keys'][0]['vault_key']='00'*32
    expect_rejected(lambda:validate('recovery-export',leaked),'raw vault key inside the export roster')
    bad=copy.deepcopy(batch); del bad['operations'][1]['parents']
    expect_rejected(lambda:validate('sync-operation',bad),'operation without parents')
    bad=copy.deepcopy(batch); bad['operations'][1]['parents']=[bad['operations'][1]['op_id']]
    expect_rejected(lambda:check_sync_batch(bad),'operation that is its own parent')
    bad=copy.deepcopy(batch); bad['operations'].append(copy.deepcopy(bad['operations'][0]))
    expect_rejected(lambda:check_sync_batch(bad),'duplicate operation id')
    bad=copy.deepcopy(batch); bad['operations'][1]['device_sequence']=9
    expect_rejected(lambda:check_sync_batch(bad),'device sequence gap')
    bad=copy.deepcopy(batch); bad['operations'][0]['device_sequence']=0
    expect_rejected(lambda:validate('sync-operation',bad),'device sequence 0')
    bad=copy.deepcopy(batch); bad['operations'][0]['payload_hash']='00'*32
    expect_rejected(lambda:check_sync_batch(bad),'payload hash mismatch')
    bad=copy.deepcopy(batch); bad['operations'][0]['change']={'kind':'note_rename','note_id':'note-john-3-16-01'}
    expect_rejected(lambda:validate('sync-operation',bad),'unknown change kind')
    bad=copy.deepcopy(batch); bad['operations'][0]['change']={'kind':'tag_add','entity_id':'note-john-3-16-01','tag_id':'Love & Grace'}
    expect_rejected(lambda:validate('sync-operation',bad),'tag name instead of a tag id')
    bad=copy.deepcopy(batch); bad['operations'][0]['signature']=bad['operations'][0]['signature'][:-2]
    expect_rejected(lambda:validate('sync-operation',bad),'truncated signature')
    bad=copy.deepcopy(roster); bad['devices'][1]['device']['device_id']='dev-'+'0'*32
    expect_rejected(lambda:check_roster(bad),'device id not derived from the signing key')
    bad=copy.deepcopy(roster); bad['devices'][2]['wrapped_keys'].append(dict(bad['devices'][0]['wrapped_keys'][1],recipient={'kind':'device','device_id':bad['devices'][2]['device']['device_id']}))
    expect_rejected(lambda:check_roster(bad),'revoked device holding the rotated key')
    bad=copy.deepcopy(roster); bad['devices'][0]['wrapped_keys'].pop()
    expect_rejected(lambda:check_roster(bad),'active device missing the current epoch')
    bad=copy.deepcopy(roster); bad['devices'][1]['approval']={'by':'device','device_id':bad['devices'][2]['device']['device_id'],'signature':'00'*64}; bad['devices'][2]['enrolled_epoch']=2
    expect_rejected(lambda:check_roster(bad),'approval by a device enrolled later')
    bad=copy.deepcopy(roster); bad['devices'][0]['approval']={'by':'recovery'}
    expect_rejected(lambda:check_roster(bad),'roster without a founder')
    bad=copy.deepcopy(roster); bad['principal']={'issuer':'mailto:someone@example.org','subject':'someone@example.org','linked_at':'2026-10-28T09:00:00Z','linked_by':bad['devices'][0]['device']['device_id']}
    expect_rejected(lambda:validate('device-enrollment',bad),'principal that is an email, not an https issuer')
    bad=copy.deepcopy(export); bad['key_epoch']=1
    expect_rejected(lambda:check_recovery_export(bad),'recovery export epochs disagree')
    bad=copy.deepcopy(export); bad['protected_secret']['argon2id']['m_cost']=1
    expect_rejected(lambda:validate('recovery-export',bad),'argon2 memory below the floor')
    bad=copy.deepcopy(sync_fixture); bad['deliveries'][0]['order'].append('op-'+'f'*64)
    expect_rejected(lambda:check_sync_fixture(bad),'delivery of an operation outside the fixture')
    print(f'PASS: {len(pairs)} schema/example validations; {len(agents)} agent records; {len(tasks)} acyclic tasks; {fixtures} conformance fixtures; source references, budgets and fixture hashes; {NEGATIVE} negative tests, including secret leakage, ink coordinates, research evidence scope, lineage and alignment shapes, quotation rights, receipt identity, key material in sync records and malformed operations.')
    print('Scope: blueprint structure only. No app, Nix build, cryptography, source clearance or Paperclip deployment has been tested.')
if __name__=='__main__': main()

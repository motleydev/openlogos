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
def check_workspace2(ws):
    # B070: workspace 2.0 (specs/22 §2). Columns hold at least one tab and show one of them; ids
    # are unique within the workspace; widths sum to one; profiles name only columns.
    columns=[c['id'] for c in ws['columns']]
    tabs=[t['id'] for c in ws['columns'] for t in c['tabs']]
    if len(set(columns))!=len(columns): raise ValueError('Duplicate column IDs')
    if len(set(tabs))!=len(tabs) or set(tabs)&set(columns): raise ValueError('Duplicate tab IDs')
    for c in ws['columns']:
        if c['shown_tab_id'] not in [t['id'] for t in c['tabs']]: raise ValueError('Shown tab is not in its column: '+c['id'])
        for t in c['tabs']:
            if t['pinned'] and t['link_group'] is not None: raise ValueError('Pinned tab inside a link group: '+t['id'])
    if ws['active_column_id'] not in columns: raise ValueError('Active column is not a column')
    if abs(sum(c['width_fraction'] for c in ws['columns'])-1)>1e-6: raise ValueError('Column widths do not sum to one')
    for cls,profile in ws.get('layout_profiles',{}).items():
        if not set(profile['visible_column_ids'])<=set(columns): raise ValueError('Profile '+cls+' names an unknown column')
        widths=profile.get('widths',{})
        if not set(widths)<=set(profile['visible_column_ids']): raise ValueError('Profile '+cls+' sizes a column it does not show')
def check_session(record, workspaces):
    # B070: a session record orders workspace records and names the active one.
    ids=[w['id'] for w in workspaces]
    if record['workspace_ids']!=ids: raise ValueError('Session does not list exactly its workspaces, in order')
    if record['active_workspace_id'] not in ids: raise ValueError('Active workspace is not open')
    for w in workspaces: check_workspace2(w)
# B040: no credential fields in portable state (specs/12, specs/03). Mirrors
# openlogos_notes::credentials; a key is credential-shaped by its words, a string value by
# its shape. The schemas of the four portable record kinds define no such property either.
CREDENTIAL_WORDS={'password','passwd','passphrase','secret','secrets','apikey','credential','credentials','bearer','cookie','cookies','oauth','jwt','privatekey','clientsecret','accesskey'}
CREDENTIAL_PAIRS={('api','key'),('private','key'),('client','secret'),('access','key'),('secret','key'),('signing','key'),('session','key')}
TOKEN_ALLOW={'token_ids','tokens','token_count','token'}
PORTABLE_SCHEMAS=['workspace','workspace-2','session','note-binding','reading-snapshot','research-thread']
def key_words(key):
    return [w.lower() for w in re.findall(r'[A-Z]+(?![a-z])|[A-Z]?[a-z0-9]+|[A-Z]+', key)]
def is_credential_key(key):
    words=key_words(key)
    if CREDENTIAL_WORDS & set(words): return True
    if any((a,b) in CREDENTIAL_PAIRS for a,b in zip(words,words[1:])): return True
    if words and words[-1] in ('token','tokens'):
        lower=key.lower()
        return lower not in TOKEN_ALLOW and not lower.endswith('_token_count')
    return False
def is_credential_value(value):
    v=value.strip()
    if '-----BEGIN' in v and 'PRIVATE KEY-----' in v: return True
    if len(v)>7 and v[:7].lower()=='bearer ': return True
    return len(v)>=20 and v.startswith(('sk-','sk_live_','sk_test_','ghp_','gho_','xoxb-','xoxp-','AKIA'))
def find_credentials(value, path=''):
    found=[]
    if isinstance(value, dict):
        for k,v in value.items():
            here=path+'/'+k
            if is_credential_key(k): found.append(here)
            found+=find_credentials(v, here)
    elif isinstance(value, list):
        for i,v in enumerate(value): found+=find_credentials(v, f'{path}/{i}')
    elif isinstance(value, str) and is_credential_value(value): found.append(path)
    return found
def check_no_credentials(doc, label):
    found=find_credentials(doc)
    if found: raise ValueError(f'Credential-shaped field in {label}: '+', '.join(found))
def schema_property_names(schema):
    names=set()
    if isinstance(schema, dict):
        for k,v in schema.get('properties',{}).items():
            names.add(k); names|=schema_property_names(v)
        for key in ('items','then','else','if','additionalProperties'):
            names|=schema_property_names(schema.get(key))
        for key in ('allOf','anyOf','oneOf'):
            for sub in schema.get(key,[]): names|=schema_property_names(sub)
        for sub in schema.get('$defs',{}).values(): names|=schema_property_names(sub)
    return names
def check_schema_has_no_credential_properties(name):
    bad=sorted(k for k in schema_property_names(load('schemas/'+name+'.schema.json')) if is_credential_key(k))
    if bad: raise ValueError(f'Schema {name} defines credential-shaped properties: '+', '.join(bad))
def check_binding(b):
    # B040: one body authority per note; a path is not identity; states carry what they need.
    if b['vault_path'] is not None: safe_path(b['vault_path'])
    if (b['vault_path'] is None) != (b['vault_id'] is None): raise ValueError('Vault path and vault id come together')
    if b['last_seen_sha256'] is not None and b['vault_path'] is None: raise ValueError('Last-seen hash without a vault path')
    st=b['state']['state']
    owner={'app_owned':'app','vault_owned':'vault','detached':'vault'}.get(st)
    if owner and b['authority']!=owner: raise ValueError(f'State {st} under {b["authority"]} authority')
    if st in ('vault_owned','conflict') and b['last_seen_sha256'] is None: raise ValueError(f'State {st} without a last-seen hash')
    if st in ('vault_owned','conflict','detached') and b['vault_path'] is None: raise ValueError(f'State {st} without a vault path')
    if st=='migrating' and b['state']['to']==b['authority']: raise ValueError('Migrating to the current authority')
    if st=='migrating' and b['state']['to']=='vault' and b['vault_path'] is None: raise ValueError('Migrating to a vault without a path')
    ops=[o['id'] for o in b.get('operations',[])]
    if len(ops)!=len(set(ops)): raise ValueError('Repeated operation id')
    for o in b.get('operations',[]):
        if (o['kind']=='delete')!=(o.get('sha256') is None): raise ValueError('Only a delete has no hash: '+o['id'])
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
def check_report(report):
    # Per-scheme minimums decide the failures, and passed follows from them (B068).
    rates={s['scheme']:(s['resolved']/(s['resolved']+s['unresolved_known_work']+s['unparsed']) if (s['resolved']+s['unresolved_known_work']+s['unparsed']) else 1.0) for s in report['schemes']}
    failing={m['scheme'] for m in report['minimums'] if m['scheme'] in rates and rates[m['scheme']]<m['minimum_resolved_rate']}
    listed={f.split(':')[0] for f in report['failures']}
    if failing!=listed: raise ValueError('Resolution report failures do not match its minimums')
    if report['passed']!=(not report['failures']): raise ValueError('Resolution report passed flag disagrees with its failures')
    canonical={}
    for s in load('examples/locator-scheme.json')['schemes']:
        if s['canonical']: canonical.setdefault(s['work'],[]).append(s['id'])
    if any(len(v)!=1 for v in canonical.values()): raise ValueError('A work must have exactly one canonical locator scheme')

def check_fixtures():
    # Conformance fixtures: id equals file name, kind known, description and license present.
    kinds={'text','versification','references','reanchor','lineage','alignment','work_citations','section_maps','workspace_layout','workspace_migration','note_authority','sync-merge','shell_commands','tool_well'}
    count=0
    for path in sorted((R/'crates/conformance/fixtures').glob('*.json')):
        f=load('crates/conformance/fixtures/'+path.name)
        if f['id']!=path.stem: raise ValueError('Fixture id differs from file name: '+path.name)
        if f['kind'] not in kinds: raise ValueError('Unknown fixture kind: '+f['kind'])
        if not f['description'] or not f['license']: raise ValueError('Fixture without description or license: '+path.name)
        if f['kind']=='sync-merge': check_sync_fixture(f)
        check_fixture_sources(f, path.name)
        count+=1
    return count

# B003: rights records (schemas/rights-record 1.1) and the source registry's pinned artifacts.
# A record describes exact bytes that live outside the repository; what the repository holds is
# the hash of their manifest and, under rights/<record>/, a copy of that manifest. The checks
# below bind those together and keep the human gate honest: a proposal is not a signature.
FIXTURE_TEXT_DENY=('ESV','English Standard Version','NIV','New International Version')
def check_rights_record(rec, label):
    if rec['status']=='approved' and (rec['reviewer'] is None or rec['reviewed_at'] is None):
        raise ValueError(f'{label}: approved without reviewer and date')
    if rec['status']=='unknown' and (rec['reviewer'] is not None or rec['reviewed_at'] is not None):
        raise ValueError(f'{label}: unknown status with a reviewer or date')
    acq=rec.get('acquisition')
    if acq is None: return
    if acq['manifest_sha256']!=rec['artifact_sha256']:
        raise ValueError(f'{label}: artifact_sha256 must be the manifest hash for a manifested artifact')
    mp=acq['manifest_path']
    if mp is not None:
        safe_path(mp)
        data=(R/mp).read_bytes()
        if hashlib.sha256(data).hexdigest()!=acq['manifest_sha256']:
            raise ValueError(f'{label}: committed manifest copy does not match manifest_sha256')
        hashes={}
        for line in data.decode('utf-8').splitlines():
            h,_,path=line.partition('  ')
            if not re.fullmatch(r'[a-f0-9]{64}',h) or not path: raise ValueError(f'{label}: malformed manifest line')
            hashes[path]=h
        if acq.get('file_count') is not None and acq['file_count']!=len(hashes):
            raise ValueError(f'{label}: file_count disagrees with the manifest')
        for lf in rec.get('license_files',[]):
            if hashes.get(lf['path'])!=lf['sha256']:
                raise ValueError(f'{label}: licence file {lf["path"]} hash is not in the manifest')
        if rec['license_artifact_sha256'] not in hashes.values():
            raise ValueError(f'{label}: license_artifact_sha256 is not a manifested file')
    if any(f['kind']=='restricted_module' for f in rec.get('flags',[])) and rec['permissions']['redistribute']=='allow' and rec.get('steward_proposal',{}).get('decision')=='restricted_module':
        raise ValueError(f'{label}: a restricted module cannot be marked redistributable')
def check_sources_pins(sources):
    for s in sources:
        pin=s['pinned_artifact']
        if pin is None: continue
        for rp in pin['rights_records']:
            safe_path(rp)
            rec=load(rp)
            if rec.get('source_id')!=s['id']: raise ValueError(f'{s["id"]}: pinned rights record {rp} names another source')
            if rec['acquisition']['commit']!=pin['commit'] and pin['commit'] is not None:
                raise ValueError(f'{s["id"]}: pin commit disagrees with {rp}')
def check_fixture_sources(fixture, label):
    text=' '.join(str(fixture.get(k,'')) for k in ('license','source','description'))
    for term in FIXTURE_TEXT_DENY:
        if re.search(r'\b'+re.escape(term)+r'\b', text):
            raise ValueError(f'{label}: fixture names a translation the rights policy excludes: {term}')

def main():
    pairs = [
        ('agents','specs/agents.json'), ('backlog','specs/backlog.json'),
        ('model-policy','specs/model-policy.json'), ('sources','specs/sources.json'),
        ('sources','examples/archive/sources-1.0.json'),
        ('claim','examples/claim.json'), ('package-manifest','examples/package-manifest.json'),
        ('rights-record','examples/rights-record.json'),
        ('rights-record','examples/archive/rights-record-1.0.json'),
        ('research-provider-policy','specs/research-provider-policy.json'),
        ('workspace-2','examples/workspace.json'),
        ('session','examples/session.json'),
        ('workspace','examples/archive/workspace-1.1.json'),
        ('workspace','examples/archive/workspace-1.0.json'),
        ('reading-snapshot','examples/reading-snapshot.json'),
        ('ink-document','examples/ink-document.json'),
        ('ink-document','examples/archive/ink-document-1.0.json'),
        ('research-thread','examples/research-thread.json'),
        ('package-manifest','examples/archive/package-manifest-1.0.json'),
        ('anchor','examples/anchor.json'), ('verse-map','examples/verse-map.json'),
        ('reference-mapping','examples/reference-mapping.json'),
        ('token-lineage','examples/token-lineage.json'), ('alignment','examples/alignment.json'),
        ('study-collection','examples/study-collection.json'),
        ('vault-export-receipt','examples/vault-export-receipt.json'),
        ('sync-operation','examples/sync-operation.json'),
        ('device-enrollment','examples/device-enrollment.json'),
        ('recovery-export','examples/recovery-export.json'),
        ('locator-scheme','examples/locator-scheme.json'),
        ('resolution-report','examples/resolution-report.json'),
        ('note-binding','examples/note-binding.json'),
        ('note-binding','examples/note-binding-conflict.json')]
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
    # Build output and installed packages are not part of the bundle (gitignored): a vendored
    # package may ship its own AGENTS.md or cite other things called S29.
    skipped={'node_modules','target','artifacts','.direnv','DerivedData','.build','dist'}
    def bundle(pattern):
        return (p for p in R.rglob(pattern) if not skipped & set(p.relative_to(R).parts[:-1]))
    for p in bundle('*.md'):
        for sid in re.findall(r'\bS\d{2}\b',p.read_text()):
            assert sid in source_ids, 'Unknown research citation: ' + sid
    for p in bundle('*'):
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
    wp=load('examples/archive/workspace-1.1.json')
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
    # B070: workspace 2.0 and the session record (specs/22 §2), the 1.x line kept under archive/.
    wp2=load('examples/workspace.json'); check_workspace2(wp2)
    session=load('examples/session.json'); check_session(session,[wp2])
    assert wp2['schema_version']=='2.0' and [[t['id'] for t in c['tabs']] for c in wp2['columns']]==[[p['id']] for p in wp['panes']], 'Workspace 2.0 example is the archived 1.1 example migrated: one column per visible pane'
    assert {'wide','regular'}==set(wp2['layout_profiles']) and wp2['layout_profiles']['regular']['visible_column_ids']==['col-p1','col-p2','col-p3']
    expect_rejected(lambda:validate('workspace-2',wp),'1.1 workspace read as 2.0')
    expect_rejected(lambda:validate('workspace',wp2),'2.0 workspace read as 1.x')
    numbered=copy.deepcopy(wp2); numbered['columns'][0]['number']=1
    expect_rejected(lambda:validate('workspace-2',numbered),'stored column number')
    no_tabs=copy.deepcopy(wp2); no_tabs['columns'][0]['tabs']=[]
    expect_rejected(lambda:validate('workspace-2',no_tabs),'column without tabs')
    stray=copy.deepcopy(wp2); stray['columns'][0]['shown_tab_id']='p2'
    expect_rejected(lambda:check_workspace2(stray),'shown tab from another column')
    dup=copy.deepcopy(wp2); dup['columns'][1]['tabs'][0]['id']='p1'; dup['columns'][1]['shown_tab_id']='p1'
    expect_rejected(lambda:check_workspace2(dup),'tab id used twice')
    lost=copy.deepcopy(wp2); lost['active_column_id']='col-gone'
    expect_rejected(lambda:check_workspace2(lost),'active column that does not exist')
    wide=copy.deepcopy(wp2); wide['columns'][0]['width_fraction']=0.5
    expect_rejected(lambda:check_workspace2(wide),'column widths that do not sum to one')
    ghost=copy.deepcopy(wp2); ghost['layout_profiles']['regular']['visible_column_ids'].append('col-gone')
    expect_rejected(lambda:check_workspace2(ghost),'profile naming an unknown column')
    pinned=copy.deepcopy(wp2); pinned['columns'][0]['tabs'][0]['pinned']=True
    expect_rejected(lambda:check_workspace2(pinned),'pinned tab in a link group')
    elsewhere=copy.deepcopy(session); elsewhere['active_workspace_id']='fixture:workspace:9'
    expect_rejected(lambda:check_session(elsewhere,[wp2]),'session whose active workspace is not open')
    bad_tool=copy.deepcopy(session); bad_tool['tool']='lasso'
    expect_rejected(lambda:validate('session',bad_tool),'unknown tool')
    assert session['reading']=={'text_size_step':0,'line_spacing':1.5,'theme':'system'} and session['ink']=={'width':4,'swatch':'#F5D90A','alternate_swatch':'#4CC38A'}, 'Session defaults follow design/tokens/shell.json'
    big_text=copy.deepcopy(session); big_text['reading']['text_size_step']=5
    expect_rejected(lambda:validate('session',big_text),'text size step beyond the seven token sizes')
    bad_spacing=copy.deepcopy(session); bad_spacing['reading']['line_spacing']=1.6
    expect_rejected(lambda:validate('session',bad_spacing),'line spacing outside 1.35, 1.5, 1.7')
    bad_swatch=copy.deepcopy(session); bad_swatch['ink']['swatch']='yellow'
    expect_rejected(lambda:validate('session',bad_swatch),'swatch that is not #RRGGBB')
    sn=load('examples/reading-snapshot.json')
    ink=load('examples/ink-document.json')
    check_ink(sn,ink)
    # B039: ink-document 1.1 adds optional per-sample tilt (altitude, azimuth; null when the
    # device has none). A 1.1 point still names its pressure; a 1.0 point carries no tilt.
    assert ink['schema_version']=='1.1' and any(p['altitude'] is not None for p in ink['strokes'][1]['points'])
    old_ink=load('examples/archive/ink-document-1.0.json'); assert old_ink['schema_version']=='1.0'
    check_ink(sn,old_ink)
    tilt_no_pressure=copy.deepcopy(ink); del tilt_no_pressure['strokes'][1]['points'][0]['pressure']
    expect_rejected(lambda:validate('ink-document',tilt_no_pressure),'1.1 ink sample with tilt but no pressure field')
    tilt_missing=copy.deepcopy(ink); del tilt_missing['strokes'][0]['points'][0]['altitude']
    expect_rejected(lambda:validate('ink-document',tilt_missing),'1.1 ink sample without the tilt fields')
    old_with_tilt=copy.deepcopy(old_ink); old_with_tilt['strokes'][0]['points'][0]['altitude']=1.0
    expect_rejected(lambda:validate('ink-document',old_with_tilt),'1.0 ink sample with 1.1 tilt')
    steep=copy.deepcopy(ink); steep['strokes'][1]['points'][0]['altitude']=2.0
    expect_rejected(lambda:validate('ink-document',steep),'ink altitude above pi/2')
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
    report=load('examples/resolution-report.json'); check_report(report)
    bad_report=copy.deepcopy(report); bad_report['passed']=True
    expect_rejected(lambda:validate('resolution-report',bad_report),'resolution report passed with failures')
    bad_report=copy.deepcopy(report); bad_report['minimums'][1]['minimum_resolved_rate']=0.5
    expect_rejected(lambda:check_report(bad_report),'resolution report failure not implied by its minimums')
    schemes=load('examples/locator-scheme.json')
    bad_schemes=copy.deepcopy(schemes); del bad_schemes['schemes'][2]['kind']['print_edition']
    expect_rejected(lambda:validate('locator-scheme',bad_schemes),'page scheme without a print edition')
    bad_schemes=copy.deepcopy(schemes); bad_schemes['schemes'][2]['canonical']=True
    expect_rejected(lambda:validate('locator-scheme',bad_schemes),'page scheme as a canonical scheme')
    # B040: workspace migration, note bindings and no credential fields in portable state.
    migration=load('crates/conformance/fixtures/workspace-migration-1-0-to-1-1.json')
    assert migration['kind']=='workspace_migration'
    for case in migration['cases']:
        validate('workspace',case['input']); check_workspace(case['input'])
        validate('workspace',case['expected']); check_workspace(case['expected'])
        assert case['input']['schema_version']==case['from'] and case['expected']['schema_version']==case['to']
        mirrored=copy.deepcopy(case['input']); mirrored['schema_version']='1.1'; mirrored.setdefault('layout_profiles',{})
        assert mirrored==case['expected'], 'Migration fixture expectation is not the 1.0 -> 1.1 step'
        assert [p['id'] for p in case['expected']['panes']]==[p['id'] for p in case['input']['panes']], 'Migration renumbered panes'
    assert migration['cases'][0]['input']==old_wp, 'Migration fixture migrates the archived 1.0 example'
    shell=load('crates/conformance/fixtures/shell-commands.json')
    assert shell['kind']=='shell_commands'
    record={'schema_version':shell['session']['schema_version'],'fixture_only':shell['session']['fixture_only'],'workspace_ids':[w['id'] for w in shell['session']['workspaces']],'active_workspace_id':shell['session']['active_workspace_id'],'tool':shell['session']['tool'],'next_serial':shell['session']['next_serial']}
    validate('session',record)
    for w in shell['session']['workspaces']: validate('workspace-2',w)
    check_session(record,shell['session']['workspaces'])
    for case in shell['migrations']:
        validate('workspace',case['input']); check_workspace(case['input'])
        validate('workspace-2',case['expected']); check_workspace2(case['expected'])
        assert [p['id'] for p in case['input']['panes']]==[t['id'] for c in case['expected']['columns'] for t in c['tabs']], 'Migration lost or reordered a pane'
    assert [c['input'] for c in shell['migrations']]==[old_wp,wp] and shell['migrations'][1]['expected']==wp2, 'Shell fixture migrates both archived examples to the 2.0 example'
    well=load('crates/conformance/fixtures/tool-well.json')
    assert well['kind']=='tool_well'
    record={'schema_version':well['session']['schema_version'],'fixture_only':well['session']['fixture_only'],'workspace_ids':[w['id'] for w in well['session']['workspaces']],'active_workspace_id':well['session']['active_workspace_id'],'tool':well['session']['tool'],'next_serial':well['session']['next_serial']}
    validate('session',record)
    for w in well['session']['workspaces']: validate('workspace-2',w); check_workspace2(w)
    check_session(record,well['session']['workspaces'])
    assert {t['kind'] for w in well['session']['workspaces'] for c in w['columns'] for t in c['tabs']}=={'reading','notes','lexicon','research'}, 'Tool well fixture has a tab of every kind'
    authority=load('crates/conformance/fixtures/note-authority-transitions.json')
    assert authority['kind']=='note_authority'
    for scenario in authority['scenarios']:
        validate('note-binding',scenario['initial']); check_binding(scenario['initial'])
        for step in scenario['steps']:
            if 'binding' in step: validate('note-binding',step['binding']); check_binding(step['binding'])
            assert ('binding' in step)!=('error' in step), 'A step ends in a binding or an error'
            check_no_credentials(step,'note authority step '+scenario['name'])
    binding=load('examples/note-binding.json'); check_binding(binding)
    conflict=load('examples/note-binding-conflict.json'); check_binding(conflict)
    assert binding['authority']=='vault' and binding['state']['state']=='vault_owned'
    assert conflict['authority']=='app' and conflict['state']['state']=='conflict' and conflict['state']['app_sha256']!=conflict['state']['vault_sha256']
    for name in PORTABLE_SCHEMAS: check_schema_has_no_credential_properties(name)
    for label,doc in [('workspace',wp2),('session',session),('workspace 1.1',wp),('workspace 1.0',old_wp),('note-binding',binding),('note-binding conflict',conflict),('reading-snapshot',sn),('research-thread',thread)]:
        check_no_credentials(doc,label)
    leaked_wp=copy.deepcopy(wp); leaked_wp['provider_token']='fixture-forbidden'
    expect_rejected(lambda:validate('workspace',leaked_wp),'token in a workspace (schema)')
    expect_rejected(lambda:check_no_credentials(leaked_wp,'workspace'),'token in a workspace (check)')
    leaked_pane=copy.deepcopy(wp); leaked_pane['panes'][0]['api_key']='fixture-forbidden'
    expect_rejected(lambda:validate('workspace',leaked_pane),'api key on a pane (schema)')
    expect_rejected(lambda:check_no_credentials(leaked_pane,'workspace'),'api key on a pane (check)')
    leaked_tab=copy.deepcopy(wp2); leaked_tab['columns'][0]['tabs'][0]['access_key']='fixture-forbidden'
    expect_rejected(lambda:validate('workspace-2',leaked_tab),'access key on a tab (schema)')
    expect_rejected(lambda:check_no_credentials(leaked_tab,'workspace'),'access key on a tab (check)')
    leaked_value=copy.deepcopy(wp); leaked_value['title']='-----BEGIN PRIVATE KEY-----\nfixture\n-----END PRIVATE KEY-----'
    expect_rejected(lambda:check_no_credentials(leaked_value,'workspace'),'private key block in an innocent field')
    leaked_binding=copy.deepcopy(binding); leaked_binding['vault_password']='fixture-forbidden'
    expect_rejected(lambda:validate('note-binding',leaked_binding),'password in a note binding (schema)')
    expect_rejected(lambda:check_no_credentials(leaked_binding,'note-binding'),'password in a note binding (check)')
    leaked_snapshot=copy.deepcopy(sn); leaked_snapshot['background']['credentials']={'user':'x'}
    expect_rejected(lambda:validate('reading-snapshot',leaked_snapshot),'credentials object in a snapshot (schema)')
    expect_rejected(lambda:check_no_credentials(leaked_snapshot,'reading-snapshot'),'credentials object in a snapshot (check)')
    expect_rejected(lambda:check_no_credentials(leaked,'research-thread'),'api key in a research thread (check)')
    leaked_thread=copy.deepcopy(thread); leaked_thread['messages'][0]['text']='Bearer fixture-forbidden-token-value'
    expect_rejected(lambda:check_no_credentials(leaked_thread,'research-thread'),'bearer header in a message body')
    bad_binding=copy.deepcopy(binding); bad_binding['authority']='app'
    expect_rejected(lambda:validate('note-binding',bad_binding),'vault_owned under app authority (schema)')
    expect_rejected(lambda:check_binding(bad_binding),'vault_owned under app authority (check)')
    bad_binding=copy.deepcopy(binding); bad_binding['vault_path']='../outside.md'
    expect_rejected(lambda:validate('note-binding',bad_binding),'binding path traversal (schema)')
    expect_rejected(lambda:check_binding(bad_binding),'binding path traversal (check)')
    bad_binding=copy.deepcopy(conflict); bad_binding['last_seen_sha256']=None
    expect_rejected(lambda:validate('note-binding',bad_binding),'conflict without a last-seen hash (schema)')
    expect_rejected(lambda:check_binding(bad_binding),'conflict without a last-seen hash (check)')
    bad_binding=copy.deepcopy(binding); bad_binding['state']={'state':'migrating','to':'vault','preview_sha256':binding['app_sha256'],'backup_id':'rev-1'}
    expect_rejected(lambda:check_binding(bad_binding),'migrating to the current authority')
    bad_binding=copy.deepcopy(binding); bad_binding['state']={'state':'migrating','to':'app','preview_sha256':binding['app_sha256']}
    expect_rejected(lambda:validate('note-binding',bad_binding),'migration without a backup')
    bad_binding=copy.deepcopy(binding); bad_binding['note_id']='Study/John 3.md'
    expect_rejected(lambda:validate('note-binding',bad_binding),'path used as note identity')
    bad_binding=copy.deepcopy(binding); bad_binding['state']={'state':'conflict','app_sha256':binding['app_sha256'],'vault_sha256':binding['app_sha256']}
    expect_rejected(lambda:validate('note-binding',bad_binding),'conflict without a cause')
    bad_binding=copy.deepcopy(binding); bad_binding['operations'].append(dict(binding['operations'][0]))
    expect_rejected(lambda:check_binding(bad_binding),'repeated operation id')
    old_migrated=copy.deepcopy(old_wp); old_migrated['layout_profiles']={}
    expect_rejected(lambda:validate('workspace',old_migrated),'migration output still declaring 1.0')
    # B003: every rights record validates, binds to its committed manifest copy, and stays unsigned
    # until a human sets status, reviewer and date; the registry's pins resolve to those records.
    rights_files=sorted(p for p in (R/'rights').glob('*.json'))
    for p in rights_files:
        rec=load('rights/'+p.name)
        validate('rights-record',rec); check_rights_record(rec,p.name)
        assert rec['id']=='rights:'+p.stem, 'Rights record id differs from file name: '+p.name
        assert rec['source_id'] in source_ids, 'Rights record names an unknown source: '+p.name
        assert rec.get('steward_proposal') is not None, 'Rights record without a steward proposal: '+p.name
    check_sources_pins(load('specs/sources.json')['sources'])
    pinned={s['id'] for s in load('specs/sources.json')['sources'] if s['pinned_artifact'] is not None}
    assert {load('rights/'+p.name)['source_id'] for p in rights_files}==pinned, 'Every rights record is pinned by its source and every pin has a record'
    assert load('examples/archive/sources-1.0.json')['schema_version']=='1.0' and all(s['pinned_artifact'] is None for s in load('examples/archive/sources-1.0.json')['sources'])
    assert 'schema_version' not in load('examples/archive/rights-record-1.0.json') and load('examples/rights-record.json')['schema_version']=='1.1'
    signed=copy.deepcopy(load('rights/'+rights_files[0].name)); signed['status']='approved'
    expect_rejected(lambda:check_rights_record(signed,'approved without reviewer'),'rights approved without reviewer')
    unsigned=copy.deepcopy(load('rights/'+rights_files[0].name)); unsigned['reviewer']='someone'
    expect_rejected(lambda:check_rights_record(unsigned,'unknown with reviewer'),'rights unknown with a reviewer')
    tampered=copy.deepcopy(load('rights/'+rights_files[0].name)); tampered['artifact_sha256']='0'*64
    expect_rejected(lambda:check_rights_record(tampered,'tampered hash'),'rights artifact hash differs from manifest hash')
    moved=copy.deepcopy(load('rights/'+rights_files[0].name)); moved['license_files'][0]['sha256']='1'*64
    expect_rejected(lambda:check_rights_record(moved,'licence hash'),'licence file hash not in manifest')
    bad_src=copy.deepcopy(load('specs/sources.json')); bad_src['sources'][0]['pinned_artifact']={'rights_records':['rights/S52.json'],'commit':None,'fetched_on':'2026-10-28','storage':'nas'}
    expect_rejected(lambda:check_sources_pins(bad_src['sources']),'pin to another source\'s record')
    bad_pin=copy.deepcopy(load('specs/sources.json')); bad_pin['sources'][0]['pinned_artifact']={'rights_records':['../outside.json'],'commit':None,'fetched_on':'2026-10-28','storage':'nas'}
    expect_rejected(lambda:validate('sources',bad_pin),'pin path outside rights/')
    bad_shape=copy.deepcopy(load('specs/sources.json')); bad_shape['sources'][0]['pinned_artifact']='rights/S52.json'
    expect_rejected(lambda:validate('sources',bad_shape),'pin as a bare string')
    esv_fixture={'license':'Text (c) 2001 Crossway, ESV','source':'x','description':'y'}
    expect_rejected(lambda:check_fixture_sources(esv_fixture,'esv fixture'),'fixture naming ESV')
    bad_flag=copy.deepcopy(load('rights/S58.json')); bad_flag['permissions']['redistribute']='allow'
    expect_rejected(lambda:check_rights_record(bad_flag,'restricted redistributable'),'restricted module marked redistributable')
    print('Scope: blueprint structure only. No app, Nix build, cryptography, source clearance or Paperclip deployment has been tested.')
    print(f'PASS: {len(pairs)} schema/example validations; {len(agents)} agent records; {len(tasks)} acyclic tasks; {fixtures} conformance fixtures; {len(rights_files)} rights records bound to their manifests and source pins; source references, budgets and fixture hashes; {NEGATIVE} negative tests, including secret leakage, credential-shaped fields in portable state, ink coordinates, research evidence scope, lineage and alignment shapes, quotation rights, receipt identity, citation resolution minimums, note-binding authority, workspace layout profiles, workspace 2.0 columns and sessions, key material in sync records, malformed operations, unsigned or tampered rights records and excluded translations in fixtures.')
if __name__=='__main__': main()

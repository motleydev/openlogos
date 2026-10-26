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
except ImportError:
    raise SystemExit('Missing jsonschema. Use a Python environment with jsonschema; no dependency is downloaded automatically.')
R = Path(__file__).resolve().parents[1]
def load(p):
    path = R / p
    if path.stat().st_size > 2_000_000:
        raise ValueError('Blueprint file exceeds validation size limit: ' + p)
    return json.loads(path.read_text())
def validate(schema_name, data):
    sch = load('schemas/' + schema_name + '.schema.json')
    Draft202012Validator.check_schema(sch)
    Draft202012Validator(sch).validate(data)
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
    try: fn()
    except (ValueError, ValidationError): return
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
        ('research-thread','examples/research-thread.json')]
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
    print(f'PASS: {len(pairs)} schema/example validations; {len(agents)} agent records; {len(tasks)} acyclic tasks; source references, budgets and fixture hashes; 9 negative tests, including secret leakage, ink coordinates and research evidence scope.')
    print('Scope: blueprint structure only. No app, Nix build, cryptography, source clearance or Paperclip deployment has been tested.')
if __name__=='__main__': main()

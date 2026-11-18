"""Regenerate the Desktop render fixtures from the real plugin and the testkit engine.

Run from the repository root after `make test-plugin` has built the testkit engine:

    uv run --no-project --python 3.12 python tests/desktop/make_fixtures.py
"""

import json, sys, time, tempfile
from pathlib import Path
sys.path.insert(0, str(Path('tests').resolve()))
import conftest  # registers the package name
from solverforge_workbench_plugin.service import Workbench
from solverforge_workbench_plugin.store import Store
from solverforge_workbench_plugin.engine_bin import StockEngine
from solverforge_workbench_plugin.tools import tool_table
tmp = Path(tempfile.mkdtemp())
T = Path('engine/target/release/workbench-testkit').resolve()
wb = Workbench(Store(tmp/'w.db'), tmp, lambda: StockEngine(tmp, override=str(T)))
tools = {s['name']: h for s, h in tool_table(lambda: wb)}
call = lambda n, a={}: json.loads(tools[n](a))
d = call('solverforge_workbench_dataset_create', {'model':'toy','demo':'','tags':['demo']})['dataset']
call('solverforge_workbench_profile_save', {'model':'toy','name':'Lenient','settings':{'weights':{'Unassigned shift':0}}})
ev = call('solverforge_workbench_evaluate', {'dataset': d['id']})['run']
r = call('solverforge_workbench_solve', {'dataset': d['id'], 'overrides': {'solve': {'seconds': 2}}})['run']
while wb.run(r['id'])['status'] in ('queued','solving'): time.sleep(0.2)
# A second dataset and a weightless profile make an experiment worth ranking.
d2 = call('solverforge_workbench_dataset_create', {'model':'toy','demo':'','name':'Demo Week B'})['dataset']
call('solverforge_workbench_profile_save', {'model':'toy','name':'Blind','settings':{'weights':{'Shift for the first nurse':0}}})
x = call('solverforge_workbench_experiment_create', {'datasets': [d['id'], d2['id']], 'profiles': ['Standard', 'Blind'],
                                                    'repeats': 1, 'overrides': {'solve': {'seconds': 30, 'best_score': '0hard/0soft'}}})['experiment']
while not wb.experiment(x['id'])['done']: time.sleep(0.2)
good = wb.dataset(d['id'], with_plan=True)['plan']
# A replan, in a store of its own so the fixtures above keep their counts: a solved demo, one of
# the model's changes on its run, the new dataset solved (lineage, changes, the run before).
tmp2 = Path(tempfile.mkdtemp())
wb2 = Workbench(Store(tmp2/'w.db'), tmp2, lambda: StockEngine(tmp2, override=str(T)))
tools2 = {s['name']: h for s, h in tool_table(lambda: wb2)}
call2 = lambda n, a={}: json.loads(tools2[n](a))
def solved(dataset):
    run = call2('solverforge_workbench_solve', {'dataset': dataset, 'overrides': {'solve': {'seconds': 1}}})['run']
    while wb2.run(run['id'])['status'] in ('queued','solving'): time.sleep(0.2)
    return run
week = call2('solverforge_workbench_dataset_create', {'model':'toy','demo':''})['dataset']
first = solved(week['id'])
child = call2('solverforge_workbench_dataset_patch', {'dataset': week['id'], 'from_run': first['id'], 'name': 'Week without shift 0',
                                                       'edits': [{'edit': 'shift_cancelled', 'args': {'shift': '0'}}]})['dataset']
r2 = solved(child['id'])
# A build as the store records one mid-way, read back through the tool.
b = wb.store.add_build(model_id='crew-tasks', name='Crew tasks', path='/home/user/crew-tasks')
wb.store.update_build(b['id'], status='building', phase='Compiling', compiled=41, total=212,
                      log='   Compiling serde v1.0.228\n   Compiling solverforge-core v0.31.1')
out = {
 'status': call('solverforge_workbench_status'), 'models': call('solverforge_workbench_models'), 'model': call('solverforge_workbench_model', {'model':'toy'}),
 'datasets': call('solverforge_workbench_datasets'), 'dataset': call('solverforge_workbench_dataset', {'dataset': d['id']}),
 'runs': call('solverforge_workbench_runs'), 'run': call('solverforge_workbench_run', {'run': r['id'], 'max_matches': 10}),
 'evaluated': call('solverforge_workbench_run', {'run': ev['id'], 'max_matches': 10}),
 'compare': call('solverforge_workbench_compare', {'runs': [ev['id'], r['id']]}),
 'fill': call('solverforge_workbench_fill', {'dataset': d['id'], 'top': 3}),
 'experiments': call('solverforge_workbench_experiments'),
 'experiment': call('solverforge_workbench_experiment', {'experiment': x['id']}),
 'trends': call('solverforge_workbench_trends', {'model': 'toy'}),
 'dataset_check': call('solverforge_workbench_dataset_check', {'model': 'toy', 'plan': good}),
 'check_broken': call('solverforge_workbench_dataset_check', {'model': 'toy', 'plan': {
     'nurses': [{'id': 0, 'name': 'solo'}], 'shifts': [{'id': 0, 'day': 0, 'nurse_idx': 4}]}}),
 'model_build': call('solverforge_workbench_model_build', {'build': b['id']}),
 'edits': call('solverforge_workbench_edits', {'run': r['id']}),
 'changes': call2('solverforge_workbench_changes', {'run': r2['id']}),
 'replanned': call2('solverforge_workbench_run', {'run': r2['id'], 'max_matches': 10}),
 'child': call2('solverforge_workbench_dataset', {'dataset': child['id']}),
}
# Fixtures are committed: absolute paths of this checkout become relative ones.
checkout = str(Path('.').resolve()) + '/'
for k, v in out.items():  # every fixture is a successful tool answer
    assert v['ok'], (k, v)
    Path(f'tests/desktop/fixtures/{k}.json').write_text(json.dumps(v, indent=1).replace(checkout, '') + '\n')
print({k: len(json.dumps(v)) for k, v in out.items()})

# The pages the backend serves for the engine-drawn views in tests/fixtures.
from solverforge_workbench_plugin import views  # noqa: E402

roster = json.loads(Path('tests/fixtures/ess-month-view.json').read_text())
Path('tests/desktop/fixtures/view.json').write_text(
    json.dumps({'ok': True, 'kind': 'timeline', 'document': views.document(roster)}) + '\n')
routes = json.loads(Path('tests/fixtures/routes-view.json').read_text())['view']
Path('tests/desktop/pages/routes.json').write_text(
    json.dumps({'ok': True, 'kind': 'routes', 'document': views.document(routes)}) + '\n')

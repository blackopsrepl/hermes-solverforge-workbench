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
out = {
 'status': call('solverforge_workbench_status'), 'models': call('solverforge_workbench_models'), 'model': call('solverforge_workbench_model', {'model':'toy'}),
 'datasets': call('solverforge_workbench_datasets'), 'dataset': call('solverforge_workbench_dataset', {'dataset': d['id']}),
 'runs': call('solverforge_workbench_runs'), 'run': call('solverforge_workbench_run', {'run': r['id'], 'max_matches': 10}),
 'evaluated': call('solverforge_workbench_run', {'run': ev['id'], 'max_matches': 10}),
 'compare': call('solverforge_workbench_compare', {'runs': [ev['id'], r['id']]}),
}
for k, v in out.items():  # every fixture is a successful tool answer
    assert v['ok'], (k, v)
    Path(f'tests/desktop/fixtures/{k}.json').write_text(json.dumps(v, indent=1) + '\n')
print({k: len(json.dumps(v)) for k, v in out.items()})

# The roster page the backend serves for the employee scheduling run in tests/fixtures.
from solverforge_workbench_plugin import views  # noqa: E402

run = json.loads(Path('tests/fixtures/ess-small-week-run.json').read_text())
page = views.document(views.timeline(run['model_id'], run['plan'], run['analysis']))
Path('tests/desktop/fixtures/view.json').write_text(json.dumps({'ok': True, 'document': page}) + '\n')

import { useEffect, useMemo, useState } from 'react';
import { Button } from '@latch/ui/button';
import { Terminal } from '@latch/ui/terminal';
import recordings from '../data/terminal-recordings.json';
const profiles = ['acme', 'personal', 'side-project'];
const scenarios = recordings.scenarios.map((scenario) => scenario.id === 'security' ? { ...scenario, title: 'Inspect & lock', steps: scenario.steps.filter((step) => step.command !== 'latch doctor --json') } : scenario);
export function LandingTerminal() {
  const [ready, setReady] = useState(false);
  const [profile, setProfile] = useState('acme');
  const [scenarioId, setScenarioId] = useState('env');
  useEffect(() => {
    setReady(true);
    const selected = document.querySelector<HTMLButtonElement>('[data-demo-profile][aria-pressed="true"]')?.dataset.demoProfile;
    if (selected && profiles.includes(selected)) setProfile(selected);
    const update = (event: Event) => { const name: unknown = (event as CustomEvent).detail; if (typeof name === 'string' && profiles.includes(name)) setProfile(name); };
    window.addEventListener('latch:demo-profile', update);
    return () => window.removeEventListener('latch:demo-profile', update);
  }, []);
  const scenario = scenarios.find((item) => item.id === scenarioId)!;
  const steps = useMemo(() => scenario.steps.map((step) => ({ command: step.command.replaceAll('acme', profile), output: step.output.map((line) => line.replaceAll('acme', profile)) })), [scenario, profile]);
  return <div className="live-terminal-demo context-terminal">
    <div className="terminal-scenarios" role="group" aria-label="Terminal scenarios">{scenarios.map((item) => <Button key={item.id} variant="ghost" size="compact" disabled={!ready} onClick={() => setScenarioId(item.id)} aria-pressed={item.id === scenarioId}>{item.title}</Button>)}</div>
    <Terminal key={`${profile}:${scenarioId}`} title={`${scenario.title} / ${profile}`} steps={steps} loop />
    <p className="terminal-provenance">Actual CLI output from a fictional vault. Project names follow the selected example.</p>
  </div>;
}

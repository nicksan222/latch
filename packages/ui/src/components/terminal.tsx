import { PauseIcon, PlayIcon, RotateCcwIcon, TerminalIcon } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { terminalDuration, terminalFrame, type TerminalStep } from '../terminal-playback';
import { Button } from './button';
import { ScrollArea } from './scroll-area';
import './terminal.css';
export interface TerminalProps {
  title: string;
  steps: readonly TerminalStep[];
  className?: string;
  loop?: boolean;
}
/** Accessible, view-aware playback of recorded command transcripts. */
export function Terminal({ title, steps, className = '', loop = false }: TerminalProps) {
  const duration = useMemo(() => terminalDuration(steps), [steps]);
  // SSR and no-JS readers receive the complete transcript. Hydration starts playback.
  const [elapsed, setElapsed] = useState(duration);
  const [paused, setPaused] = useState(false);
  const [inView, setInView] = useState(false);
  const [visible, setVisible] = useState(true);
  const [reduced, setReduced] = useState(false);
  const [following, setFollowing] = useState(true);
  const root = useRef<HTMLElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const frame = useMemo(() => terminalFrame(steps, elapsed), [steps, elapsed]);
  const playing = !paused && inView && visible && !reduced && !frame.complete;
  useEffect(() => {
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    const applyMotion = () => {
      setReduced(motion.matches);
      setElapsed(motion.matches ? duration : 0);
      setPaused(false);
    };
    applyMotion();
    motion.addEventListener('change', applyMotion);
    return () => motion.removeEventListener('change', applyMotion);
  }, [duration, steps]);
  useEffect(() => {
    const update = () => setVisible(document.visibilityState !== 'hidden');
    update();
    document.addEventListener('visibilitychange', update);
    const observer = new IntersectionObserver(
      ([entry]) => setInView(Boolean(entry?.isIntersecting)),
      { threshold: 0.15 }
    );
    if (root.current) observer.observe(root.current);
    return () => {
      document.removeEventListener('visibilitychange', update);
      observer.disconnect();
    };
  }, []);
  useEffect(() => {
    if (!loop || !frame.complete || paused || !inView || !visible || reduced || !following) return;
    const timer = setTimeout(() => setElapsed(0), 2800);
    return () => clearTimeout(timer);
  }, [loop, frame.complete, paused, inView, visible, reduced, following]);
  useEffect(() => {
    if (!playing) return;
    const timer = setTimeout(() => setElapsed((value) => Math.min(duration, value + 40)), 40);
    return () => clearTimeout(timer);
  }, [playing, elapsed, duration]);
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const update = () =>
      setFollowing(element.scrollHeight - element.scrollTop - element.clientHeight < 18);
    element.addEventListener('scroll', update, { passive: true });
    return () => element.removeEventListener('scroll', update);
  }, []);
  useEffect(() => {
    if (viewport.current && playing && following)
      viewport.current.scrollTop = viewport.current.scrollHeight;
  }, [frame.lines.length, playing, following]);
  const status = paused ? 'Paused' : frame.complete ? 'Complete' : playing ? 'Playing' : 'Ready';
  const replay = () => {
    setElapsed(reduced ? duration : 0);
    setPaused(false);
    setFollowing(true);
  };
  return (
    <section
      ref={root}
      className={`latch-terminal ${playing ? 'is-playing' : ''} ${className}`}
      aria-label={`${title} terminal demo`}
    >
      <div className="latch-terminal-toolbar">
        <span className="latch-terminal-title">
          <TerminalIcon size={16} aria-hidden />
          {title}
        </span>
        <div className="latch-terminal-controls">
          <span className="latch-terminal-status" role="status" aria-live="off">
            {status}
          </span>
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={reduced || (frame.complete && !loop)}
            onClick={() => setPaused((value) => !value)}
            aria-label={paused ? 'Resume terminal playback' : 'Pause terminal playback'}
          >
            {paused ? <PlayIcon aria-hidden /> : <PauseIcon aria-hidden />}
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={replay} aria-label="Replay terminal demo">
            <RotateCcwIcon aria-hidden />
          </Button>
        </div>
      </div>
      <ScrollArea
        viewportRef={viewport}
        className="latch-terminal-viewport"
        viewportTabIndex={0}
        role="region"
        aria-label="Scrollable terminal output"
      >
        <div className="latch-terminal-content">
          <div className="latch-terminal-lines" aria-hidden="true">
            {frame.lines.map((line) => (
              <div key={line.id} className={`latch-terminal-line is-${line.kind}`}>
                <span className="latch-terminal-prompt">{line.kind === 'command' ? '$' : ''}</span>
                <span>
                  {line.text}
                  {line.pending && <span className="latch-terminal-cursor" />}
                </span>
              </div>
            ))}
          </div>
          <pre className="sr-only">
            {steps.map((step) => `$ ${step.command}\n${step.output.join('\n')}`).join('\n\n')}
          </pre>
        </div>
      </ScrollArea>
      <div className="latch-terminal-footer">
        {following ? (
          <span>Recorded CLI session</span>
        ) : (
          <Button
            size="compact"
            variant="ghost"
            onClick={() => {
              if (viewport.current) viewport.current.scrollTop = viewport.current.scrollHeight;
              setFollowing(true);
            }}
          >
            Follow output
          </Button>
        )}
        <progress
          aria-label="Terminal playback progress"
          max={Math.max(1, duration)}
          value={elapsed}
        />
        <span>{frame.complete ? 'done' : `${Math.floor(elapsed / 1000)}s`}</span>
      </div>
    </section>
  );
}

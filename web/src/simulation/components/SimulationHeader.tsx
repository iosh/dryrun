import { Badge } from '../../ui/Badge.tsx';

export interface SimulationHeaderProps {
  networkLabel: string;
}

export function SimulationHeader({
  networkLabel,
}: Readonly<SimulationHeaderProps>) {
  return (
    <>
      <header className="flex h-16 items-center justify-between border-b border-line bg-white px-5 sm:px-6">
        <div className="flex items-center gap-4">
          <div className="font-display text-[22px] font-bold text-ink-950">
            dryrun
          </div>
          <div className="hidden items-center gap-2 sm:flex">
            <Badge tone="slate">Ethereum</Badge>
            <Badge tone="blue">{networkLabel}</Badge>
          </div>
        </div>
        <div className="sm:hidden">
          <Badge tone="blue">{networkLabel}</Badge>
        </div>
      </header>
      <div className="flex min-h-7 items-center border-b border-line bg-shell-100 px-5 sm:px-6">
        <p className="font-mono text-[10px] font-medium uppercase tracking-[0.16em] text-ink-600">
          Simulate a transaction, inspect execution, and review detected changes
        </p>
      </div>
    </>
  );
}

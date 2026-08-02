import { cn } from '../../../lib/cn.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '../../../ui/Tooltip.tsx';
import { normalizeAddress } from '../../flowView.ts';
import type { AddressHighlightController } from './resultTypes.ts';

export function AddressValue({
  address,
  addressHighlight,
}: Readonly<{
  address: string;
  addressHighlight: AddressHighlightController;
}>) {
  const normalized = normalizeAddress(address);
  const active = addressHighlight.activeAddress === normalized;

  return (
    <div className="flex min-w-0 items-start gap-1">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            aria-label={`Highlight matching address ${address}`}
            aria-pressed={addressHighlight.pinnedAddress === normalized}
            className={cn(
              'min-w-0 flex-1 break-all rounded px-1 py-0.5 text-left font-mono text-[11px] leading-5 text-ink-600 transition-colors hover:bg-brand-50 hover:text-brand-700 focus-visible:bg-brand-50',
              active && 'bg-amber-100 text-amber-900 ring-1 ring-amber-300',
            )}
            data-address-value=""
            onBlur={addressHighlight.onAddressLeave}
            onClick={() => addressHighlight.onAddressToggle(normalized)}
            onFocus={() => addressHighlight.onAddressEnter(normalized)}
            onMouseEnter={() => addressHighlight.onAddressEnter(normalized)}
            onMouseLeave={addressHighlight.onAddressLeave}
            type="button"
          >
            {address}
          </button>
        </TooltipTrigger>
        <TooltipContent>Highlight matching address</TooltipContent>
      </Tooltip>
      <CopyButton label="Copy address" value={address} />
    </div>
  );
}

export function AddressAliasValue({
  address,
  addressHighlight,
  label,
}: Readonly<{
  address: string;
  addressHighlight: AddressHighlightController;
  label: string;
}>) {
  const normalized = normalizeAddress(address);
  const active = addressHighlight.activeAddress === normalized;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          aria-label={`Highlight ${label}: ${address}`}
          aria-pressed={addressHighlight.pinnedAddress === normalized}
          className={cn(
            'min-w-10 rounded border border-line bg-white px-2.5 py-1.5 text-xs font-semibold text-ink-600 transition-colors hover:border-brand-600/30 hover:bg-brand-50 hover:text-brand-700',
            active && 'border-amber-300 bg-amber-100 text-amber-900',
          )}
          data-address-value=""
          onBlur={addressHighlight.onAddressLeave}
          onClick={() => addressHighlight.onAddressToggle(normalized)}
          onFocus={() => addressHighlight.onAddressEnter(normalized)}
          onMouseEnter={() => addressHighlight.onAddressEnter(normalized)}
          onMouseLeave={addressHighlight.onAddressLeave}
          type="button"
        >
          {label}
        </button>
      </TooltipTrigger>
      <TooltipContent className="font-mono">{address}</TooltipContent>
    </Tooltip>
  );
}

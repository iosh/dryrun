import { cn } from '../../../lib/cn.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '../../../ui/Tooltip.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

export function AddressValue({
  address,
  addressHighlight,
}: Readonly<{
  address: string;
  addressHighlight: AddressHighlightController;
}>) {
  const normalized = address.toLowerCase();
  const active = addressHighlight.activeAddress === normalized;

  return (
    <div className="flex min-w-0 items-start gap-1">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            aria-label={`Highlight matching address ${address}`}
            aria-pressed={addressHighlight.pinnedAddress === normalized}
            className={cn(
              'min-w-0 flex-1 break-all rounded px-1 py-0.5 text-left font-mono text-[11px] leading-5 transition-colors',
              active
                ? 'bg-amber-100 text-amber-900 ring-1 ring-amber-300'
                : 'text-ink-600 hover:bg-brand-50 hover:text-brand-700 focus-visible:bg-brand-50',
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

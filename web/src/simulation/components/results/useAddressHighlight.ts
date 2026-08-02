import { useState } from 'react';

import type { AddressHighlightController } from './resultTypes.ts';

export function useAddressHighlight(): AddressHighlightController {
  const [hoveredAddress, setHoveredAddress] = useState<string | null>(null);
  const [pinnedAddress, setPinnedAddress] = useState<string | null>(null);

  return {
    activeAddress: hoveredAddress ?? pinnedAddress,
    clearPinnedAddress: () => setPinnedAddress(null),
    onAddressEnter: setHoveredAddress,
    onAddressLeave: () => setHoveredAddress(null),
    onAddressToggle: (address) =>
      setPinnedAddress((current) =>
        current === address ? null : address,
      ),
    pinnedAddress,
  };
}

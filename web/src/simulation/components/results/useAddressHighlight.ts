import { useState } from 'react';

export interface AddressHighlightController {
  activeAddress: string | null;
  pinnedAddress: string | null;
  clearPinnedAddress: () => void;
  onAddressEnter: (address: string) => void;
  onAddressLeave: () => void;
  onAddressToggle: (address: string) => void;
}

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

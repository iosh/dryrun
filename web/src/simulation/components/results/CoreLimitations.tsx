const LIMITATIONS: Record<string, string> = {
  storageOwnershipApproximated: 'Storage ownership is approximated. Exact ownership changes are unavailable.',
  storageCollateralSkipped: 'Storage collateral settlement and funding checks are skipped. A transaction that fails on-chain for insufficient collateral may succeed here.',
  storagePointsApproximated: 'Storage points are approximated, which can affect sponsorship behavior.',
  storagePointsInitializationAssumed: 'Existing contracts are treated as initialized for storage points. Sponsorship behavior can differ.',
  sponsorWhitelistApproximated: 'Hidden sponsorship whitelist entries may be missing, which can affect permissions and contract behavior.',
  contractCleanupPartial: 'Old storage collateral refunds and older whitelist cleanup are omitted during contract deletion.',
};

export function CoreLimitations({ limitations }: Readonly<{ limitations: string[] }>) {
  return (
    <section className="rounded-lg border border-amber-200 bg-amber-50 px-5 py-4 text-amber-950">
      <h3 className="text-sm font-semibold">Approximate Core Space simulation</h3>
      <p className="mt-1 text-xs leading-5">These limits can affect execution behavior and reported changes.</p>
      <ul className="mt-2 list-disc space-y-1 pl-4 text-xs leading-5">
        {limitations.map((limitation) => <li key={limitation}>{LIMITATIONS[limitation] ?? limitation}</li>)}
      </ul>
    </section>
  );
}

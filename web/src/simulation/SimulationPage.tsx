import { SimulationComposer } from './components/SimulationComposer.tsx';
import { SimulationHeader } from './components/SimulationHeader.tsx';
import {
  SimulationHistoryMobile,
  SimulationHistorySidebar,
} from './components/SimulationHistory.tsx';
import { SimulationResults } from './components/results/SimulationResults.tsx';
import { useSimulationPage } from './useSimulationPage.ts';

export function SimulationPage() {
  const page = useSimulationPage();

  return (
    <main className="min-h-screen bg-shell-50 text-ink-950">
      <SimulationHeader
        environmentId={page.environmentId}
        isBusy={page.isRunning}
        mobileHistoryAction={
          <SimulationHistoryMobile
            activeRecordId={page.activeRecord?.id ?? null}
            history={page.history}
            isBusy={page.isRunning}
            onDeleteHistoryEntry={page.deleteHistoryEntry}
            onNewSimulation={page.startNewSimulation}
            onSelectHistoryEntry={page.selectHistoryEntry}
          />
        }
        onEnvironmentChange={page.changeEnvironment}
      />

      <div className="mx-auto grid min-h-[calc(100vh-73px)] max-w-450 lg:grid-cols-[252px_minmax(400px,500px)_minmax(520px,1fr)]">
        <SimulationHistorySidebar
          activeRecordId={page.activeRecord?.id ?? null}
          history={page.history}
          isBusy={page.isRunning}
          onDeleteHistoryEntry={page.deleteHistoryEntry}
          onNewSimulation={page.startNewSimulation}
          onSelectHistoryEntry={page.selectHistoryEntry}
        />

        <section className="self-start border-line bg-white px-4 py-5 sm:px-6 lg:border-r lg:px-7 lg:py-7">
          <SimulationComposer
            environmentId={page.environmentId}
            form={page.form}
            isRunning={page.isRunning}
            onReset={page.startNewSimulation}
          />
        </section>

        <section className="min-w-0 border-t border-line bg-shell-50 px-4 py-5 sm:px-6 lg:border-t-0 lg:px-8 lg:py-7">
          <SimulationResults
            activeRecord={page.activeRecord}
            isRunning={page.isRunning}
            runError={page.runError}
          />
        </section>
      </div>
    </main>
  );
}

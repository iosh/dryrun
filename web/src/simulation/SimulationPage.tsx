import { SimulationComposer } from './components/SimulationComposer.tsx';
import { SimulationHeader } from './components/SimulationHeader.tsx';
import {
  SimulationHistoryMobileList,
  SimulationHistoryMobileToolbar,
  SimulationHistorySidebar,
} from './components/SimulationHistory.tsx';
import { SimulationResults } from './components/SimulationResults.tsx';
import { useSimulationPage } from './useSimulationPage.ts';

export function SimulationPage() {
  const {
    activeRecord,
    activeResponseJson,
    changeItems,
    form,
    history,
    isRunning,
    resetComposer,
    runError,
    selectedHistoryId,
    selectHistoryEntry,
    startNewSimulation,
  } = useSimulationPage();

  return (
    <main className="min-h-screen bg-shell-50 text-ink-950">
      <div className="min-h-screen bg-shell-50">
        <SimulationHeader networkLabel="Mainnet" />
        <SimulationHistoryMobileToolbar
          isBusy={isRunning}
          onNewSimulation={startNewSimulation}
        />

        <div className="grid min-h-[calc(100vh-92px)] lg:grid-cols-[240px_minmax(440px,560px)_1fr]">
          <SimulationHistorySidebar
            history={history}
            isBusy={isRunning}
            onNewSimulation={startNewSimulation}
            onSelectHistoryEntry={selectHistoryEntry}
            selectedHistoryId={selectedHistoryId}
          />

          <div className="border-r-0 bg-shell-50 p-4 sm:p-5 lg:border-r lg:border-line lg:p-6">
            <SimulationComposer
              form={form}
              isRunning={isRunning}
              onReset={resetComposer}
            />
          </div>

          <div className="bg-shell-50 p-4 pt-0 sm:p-5 sm:pt-0 lg:p-6">
            <SimulationResults
              activeRecord={activeRecord}
              changeItems={changeItems}
              rawResponseJson={activeResponseJson}
              runError={runError}
            />
            <div className="mt-4">
              <SimulationHistoryMobileList
                history={history}
                onSelectHistoryEntry={selectHistoryEntry}
                selectedHistoryId={selectedHistoryId}
              />
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}

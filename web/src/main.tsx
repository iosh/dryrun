import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import App from './App.tsx';
import './index.css';
import { TooltipProvider } from './ui/Tooltip.tsx';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <TooltipProvider delayDuration={350}>
      <App />
    </TooltipProvider>
  </StrictMode>,
);

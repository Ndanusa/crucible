import { useState } from 'react';
import { EventListenerDashboard } from './components/EventListenerDashboard';
import { TransactionSimulator } from './components/TransactionSimulator';
import './App.css';

function App() {
  const [activeTab, setActiveTab] = useState<'events' | 'simulator'>('events');

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-brand">
          <h1>Crucible Dashboard</h1>
          <div className="header-badge">Mainnet Beta</div>
        </div>

        <nav className="header-tabs" aria-label="Dashboard views">
          <button
            type="button"
            className={`tab-btn ${activeTab === 'events' ? 'active' : ''}`}
            onClick={() => setActiveTab('events')}
            data-testid="tab-events"
          >
            Event Listener
          </button>
          <button
            type="button"
            className={`tab-btn ${activeTab === 'simulator' ? 'active' : ''}`}
            onClick={() => setActiveTab('simulator')}
            data-testid="tab-simulator"
          >
            Transaction Simulator
          </button>
        </nav>
      </header>
      
      <main className="app-main">
        {activeTab === 'events' ? <EventListenerDashboard /> : <TransactionSimulator />}
      </main>
    </div>
  );
}

export default App;

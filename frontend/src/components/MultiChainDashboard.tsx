import React, { useState, useEffect } from 'react';
import { Globe, Plus, Trash2, CheckCircle2, AlertTriangle, Cpu, Link2, Key, Activity } from 'lucide-react';
import './MultiChainDashboard.css';

interface Network {
  id: string;
  name: string;
  rpcUrl: string;
  passphrase: string;
  status: 'online' | 'offline';
  pingMs: number;
  latestLedger: number;
  activeContractsCount: number;
  isCustom?: boolean;
}

const DEFAULT_NETWORKS: Network[] = [
  {
    id: 'mainnet',
    name: 'Soroban Mainnet',
    rpcUrl: 'https://soroban-mainnet.stellar.org:443',
    passphrase: 'Public Global Stellar Network ; October 2015',
    status: 'online',
    pingMs: 82,
    latestLedger: 1045231,
    activeContractsCount: 1420,
  },
  {
    id: 'testnet',
    name: 'Soroban Testnet',
    rpcUrl: 'https://soroban-testnet.stellar.org',
    passphrase: 'Test SDF Network ; September 2015',
    status: 'online',
    pingMs: 34,
    latestLedger: 452934,
    activeContractsCount: 328,
  },
  {
    id: 'futurenet',
    name: 'Soroban Futurenet',
    rpcUrl: 'https://rpc-futurenet.stellar.org',
    passphrase: 'Test SDF Future Network ; October 2022',
    status: 'online',
    pingMs: 56,
    latestLedger: 98124,
    activeContractsCount: 94,
  },
  {
    id: 'sandbox',
    name: 'Local Sandbox',
    rpcUrl: 'http://localhost:8000',
    passphrase: 'Standalone Network ; Standalone',
    status: 'online',
    pingMs: 2,
    latestLedger: 4239,
    activeContractsCount: 15,
  },
];

export const MultiChainDashboard: React.FC = () => {
  const [networks, setNetworks] = useState<Network[]>(DEFAULT_NETWORKS);
  const [selectedNetwork, setSelectedNetwork] = useState<string>('testnet');
  const [showAddForm, setShowAddForm] = useState<boolean>(false);
  
  // Form fields
  const [newName, setNewName] = useState<string>('');
  const [newRpc, setNewRpc] = useState<string>('');
  const [newPassphrase, setNewPassphrase] = useState<string>('');

  useEffect(() => {
    const saved = localStorage.getItem('crucible_custom_networks');
    if (saved) {
      try {
        const parsed = JSON.parse(saved) as Network[];
        setNetworks([...DEFAULT_NETWORKS, ...parsed]);
      } catch (e) {
        console.error('Failed to parse custom networks', e);
      }
    }
  }, []);

  const handleAddNetwork = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName || !newRpc || !newPassphrase) return;

    const newNet: Network = {
      id: `custom-${Date.now()}`,
      name: newName,
      rpcUrl: newRpc,
      passphrase: newPassphrase,
      status: 'online',
      pingMs: Math.floor(Math.random() * 40) + 5,
      latestLedger: Math.floor(Math.random() * 5000) + 100,
      activeContractsCount: 0,
      isCustom: true,
    };

    const updated = [...networks.filter(n => n.isCustom), newNet];
    localStorage.setItem('crucible_custom_networks', JSON.stringify(updated));
    setNetworks([...DEFAULT_NETWORKS, ...updated]);
    setSelectedNetwork(newNet.id);
    
    // Reset form
    setNewName('');
    setNewRpc('');
    setNewPassphrase('');
    setShowAddForm(false);
  };

  const handleDeleteNetwork = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const customOnly = networks.filter(n => n.isCustom && n.id !== id);
    localStorage.setItem('crucible_custom_networks', JSON.stringify(customOnly));
    setNetworks([...DEFAULT_NETWORKS, ...customOnly]);
    if (selectedNetwork === id) {
      setSelectedNetwork('testnet');
    }
  };

  const selectedNetObj = networks.find(n => n.id === selectedNetwork) || networks[1];

  return (
    <div className="multichain-container">
      <div className="multichain-header">
        <div className="header-icon-wrapper">
          <Globe className="header-icon" />
        </div>
        <div>
          <h2>Multi-Chain Support</h2>
          <p>Deploy, monitor and switch between Stellar and Soroban network nodes</p>
        </div>
      </div>

      <div className="multichain-content">
        <div className="networks-panel glass-panel">
          <div className="panel-header">
            <h3 className="section-title">Configured Networks</h3>
            <button 
              className="add-network-trigger"
              onClick={() => setShowAddForm(!showAddForm)}
              data-testid="add-network-toggle"
            >
              <Plus size={16} />
              Add Network
            </button>
          </div>

          {showAddForm && (
            <form onSubmit={handleAddNetwork} className="add-network-form glass-panel" data-testid="add-network-form">
              <h4>Add Custom Network</h4>
              <div className="form-group">
                <label htmlFor="network-name">Network Name</label>
                <input 
                  id="network-name"
                  type="text" 
                  value={newName} 
                  onChange={e => setNewName(e.target.value)} 
                  placeholder="e.g. My Local Validator"
                  required
                />
              </div>
              <div className="form-group">
                <label htmlFor="rpc-url">RPC Node URL</label>
                <input 
                  id="rpc-url"
                  type="url" 
                  value={newRpc} 
                  onChange={e => setNewRpc(e.target.value)} 
                  placeholder="http://127.0.0.1:8000"
                  required
                />
              </div>
              <div className="form-group">
                <label htmlFor="passphrase">Network Passphrase</label>
                <input 
                  id="passphrase"
                  type="text" 
                  value={newPassphrase} 
                  onChange={e => setNewPassphrase(e.target.value)} 
                  placeholder="e.g. Standalone Network ; Standalone"
                  required
                />
              </div>
              <div className="form-actions">
                <button type="button" className="btn-cancel" onClick={() => setShowAddForm(false)}>Cancel</button>
                <button type="submit" className="btn-submit">Add Node</button>
              </div>
            </form>
          )}

          <div className="networks-list">
            {networks.map(net => (
              <div 
                key={net.id}
                className={`network-card ${selectedNetwork === net.id ? 'active' : ''}`}
                onClick={() => setSelectedNetwork(net.id)}
                data-testid={`network-card-${net.id}`}
              >
                <div className="card-top">
                  <div className="net-info">
                    <span className="net-name">{net.name}</span>
                    {net.isCustom && <span className="custom-badge">Custom</span>}
                  </div>
                  <div className="status-indicator">
                    <span className={`status-dot ${net.status}`}></span>
                    <span className="status-text">{net.status}</span>
                  </div>
                </div>

                <div className="card-details">
                  <div className="detail-row">
                    <Link2 size={12} className="detail-icon" />
                    <span className="detail-val truncate">{net.rpcUrl}</span>
                  </div>
                </div>

                {net.isCustom && (
                  <button 
                    className="delete-net-btn"
                    onClick={(e) => handleDeleteNetwork(net.id, e)}
                    data-testid={`delete-network-${net.id}`}
                    aria-label={`Delete network ${net.name}`}
                  >
                    <Trash2 size={14} />
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>

        <div className="active-network-details glass-panel" data-testid="active-network-details">
          <div className="details-header">
            <Cpu className="details-header-icon" />
            <div>
              <h3>Active Network Connection</h3>
              <p className="active-net-name">{selectedNetObj.name}</p>
            </div>
          </div>

          <div className="network-meta-grid">
            <div className="meta-card">
              <Activity size={18} className="meta-icon" />
              <div className="meta-info">
                <span className="meta-label">Node Ping</span>
                <span className="meta-value">{selectedNetObj.pingMs} ms</span>
              </div>
            </div>

            <div className="meta-card">
              <CheckCircle2 size={18} className="meta-icon" />
              <div className="meta-info">
                <span className="meta-label">Latest Ledger</span>
                <span className="meta-value">#{selectedNetObj.latestLedger.toLocaleString()}</span>
              </div>
            </div>

            <div className="meta-card">
              <AlertTriangle size={18} className="meta-icon" />
              <div className="meta-info">
                <span className="meta-label">Active Contracts</span>
                <span className="meta-value">{selectedNetObj.activeContractsCount}</span>
              </div>
            </div>
          </div>

          <div className="config-block">
            <h4>Node Endpoint Config</h4>
            <div className="config-item">
              <div className="config-label"><Link2 size={12} /> RPC Endpoint</div>
              <div className="config-value" data-testid="details-rpc-url">{selectedNetObj.rpcUrl}</div>
            </div>
            <div className="config-item">
              <div className="config-label"><Key size={12} /> Network Passphrase</div>
              <div className="config-value passphrase" data-testid="details-passphrase">{selectedNetObj.passphrase}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

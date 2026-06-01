import React, { useState, useMemo } from 'react';
import { 
  XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Area, AreaChart 
} from 'recharts';
import { Zap, Activity, Cpu, Shield, ArrowRight } from 'lucide-react';
import './GasCostEstimator.css';

// Mock data for the visualization
const historicalData = [
  { time: '10:00', cost: 1200 },
  { time: '10:05', cost: 1400 },
  { time: '10:10', cost: 1350 },
  { time: '10:15', cost: 1600 },
  { time: '10:20', cost: 1550 },
  { time: '10:25', cost: 1800 },
  { time: '10:30', cost: 1750 },
];

const contractTypes = [
  { id: 'token', name: 'Token Transfer', baseGas: 1500, risk: 'Low' },
  { id: 'nft', name: 'NFT Mint', baseGas: 4500, risk: 'Medium' },
  { id: 'defi', name: 'DeFi Swap', baseGas: 8000, risk: 'High' },
  { id: 'custom', name: 'Custom Logic', baseGas: 12000, risk: 'High' }
];

export const GasCostEstimator: React.FC = () => {
  const [selectedContract, setSelectedContract] = useState(contractTypes[0].id);
  const [complexity, setComplexity] = useState(1);
  const [isSimulating, setIsSimulating] = useState(false);

  const contract = contractTypes.find(c => c.id === selectedContract)!;
  
  const estimatedCost = useMemo(() => {
    return Math.floor(contract.baseGas * complexity * (1 + (Math.random() * 0.1)));
  }, [contract.baseGas, complexity, isSimulating]); // added isSimulating to trigger re-calc on simulate

  const handleSimulate = () => {
    setIsSimulating(true);
    setTimeout(() => {
      setIsSimulating(false);
    }, 800);
  };

  const chartData = useMemo(() => {
    return historicalData.map(d => ({
      ...d,
      projected: d.cost * (complexity * 0.8 + 0.2)
    }));
  }, [complexity]);

  return (
    <div className="gas-estimator-container">
      <div className="gas-estimator-header">
        <div className="header-icon-wrapper">
          <Zap className="header-icon" />
        </div>
        <div>
          <h2>Gas Cost Estimator</h2>
          <p>Real-time predictive analysis for Soroban contracts</p>
        </div>
      </div>

      <div className="gas-estimator-content">
        <div className="controls-section glass-panel">
          <h3 className="section-title">Configuration</h3>
          
          <div className="form-group">
            <label>Contract Type</label>
            <div className="contract-selector">
              {contractTypes.map(c => (
                <button
                  key={c.id}
                  className={`contract-btn ${selectedContract === c.id ? 'active' : ''}`}
                  onClick={() => setSelectedContract(c.id)}
                  data-testid={`select-${c.id}`}
                >
                  {c.name}
                </button>
              ))}
            </div>
          </div>

          <div className="form-group">
            <label>
              Complexity Multiplier: <span>{complexity.toFixed(1)}x</span>
            </label>
            <input 
              type="range" 
              min="1" 
              max="5" 
              step="0.1"
              value={complexity}
              onChange={(e) => setComplexity(parseFloat(e.target.value))}
              className="complexity-slider"
              data-testid="complexity-slider"
            />
          </div>

          <div className="stats-grid">
            <div className="stat-card">
              <Activity className="stat-icon" size={16} />
              <div className="stat-info">
                <span className="stat-label">Base Cost</span>
                <span className="stat-value">{contract.baseGas}</span>
              </div>
            </div>
            <div className="stat-card">
              <Shield className="stat-icon" size={16} />
              <div className="stat-info">
                <span className="stat-label">Risk Level</span>
                <span className={`stat-value risk-${contract.risk.toLowerCase()}`}>
                  {contract.risk}
                </span>
              </div>
            </div>
          </div>

          <button 
            className={`simulate-btn ${isSimulating ? 'simulating' : ''}`}
            onClick={handleSimulate}
            disabled={isSimulating}
            data-testid="simulate-btn"
          >
            {isSimulating ? 'Analyzing...' : 'Run Simulation'}
            {!isSimulating && <ArrowRight size={16} />}
          </button>
        </div>

        <div className="visualization-section glass-panel">
          <div className="estimation-result">
            <div className="result-header">
              <Cpu className="result-icon" />
              <h3>Estimated Execution Cost</h3>
            </div>
            <div className="result-value" data-testid="estimated-cost">
              {estimatedCost.toLocaleString()} <span className="unit">stroops</span>
            </div>
          </div>

          <div className="chart-container" data-testid="chart-container">
            <h4 className="chart-title">Cost Projection Analysis</h4>
            <ResponsiveContainer width="100%" height={250}>
              <AreaChart data={chartData} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                <defs>
                  <linearGradient id="colorCost" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#8b5cf6" stopOpacity={0.3}/>
                    <stop offset="95%" stopColor="#8b5cf6" stopOpacity={0}/>
                  </linearGradient>
                  <linearGradient id="colorProjected" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#06b6d4" stopOpacity={0.3}/>
                    <stop offset="95%" stopColor="#06b6d4" stopOpacity={0}/>
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#334155" vertical={false} />
                <XAxis dataKey="time" stroke="#94a3b8" fontSize={12} tickLine={false} axisLine={false} />
                <YAxis stroke="#94a3b8" fontSize={12} tickLine={false} axisLine={false} tickFormatter={(val) => `${val/1000}k`} />
                <Tooltip 
                  contentStyle={{ backgroundColor: 'rgba(15, 23, 42, 0.9)', borderColor: '#334155', borderRadius: '8px' }}
                  itemStyle={{ color: '#e2e8f0' }}
                />
                <Area type="monotone" dataKey="cost" stroke="#8b5cf6" strokeWidth={2} fillOpacity={1} fill="url(#colorCost)" name="Historical Cost" />
                <Area type="monotone" dataKey="projected" stroke="#06b6d4" strokeWidth={2} fillOpacity={1} fill="url(#colorProjected)" name="Projected Cost" />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>
    </div>
  );
};


import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { GasCostEstimator } from './GasCostEstimator';

import { vi } from 'vitest';

// Mock Recharts to avoid issues in jsdom
vi.mock('recharts', async () => {
  const OriginalRecharts = await vi.importActual('recharts') as any;
  return {
    ...OriginalRecharts,
    ResponsiveContainer: ({ children }: any) => <div>{children}</div>,
    LineChart: ({ children }: any) => <div data-testid="line-chart">{children}</div>,
    AreaChart: ({ children }: any) => <div data-testid="area-chart">{children}</div>,
  };
});

describe('GasCostEstimator', () => {
  it('renders correctly', () => {
    render(<GasCostEstimator />);
    
    expect(screen.getByText('Gas Cost Estimator')).toBeInTheDocument();
    expect(screen.getByText('Real-time predictive analysis for Soroban contracts')).toBeInTheDocument();
    expect(screen.getByTestId('area-chart')).toBeInTheDocument();
  });

  it('changes contract type when clicked', () => {
    render(<GasCostEstimator />);
    
    const nftBtn = screen.getByTestId('select-nft');
    fireEvent.click(nftBtn);
    
    expect(nftBtn).toHaveClass('active');
    
    // Base cost for NFT should be 4500
    expect(screen.getByText('4500')).toBeInTheDocument();
    // Risk level Medium
    expect(screen.getByText('Medium')).toBeInTheDocument();
  });

  it('updates complexity multiplier on slider change', () => {
    render(<GasCostEstimator />);
    
    const slider = screen.getByTestId('complexity-slider');
    
    fireEvent.change(slider, { target: { value: '2.5' } });
    expect(screen.getByText('2.5x')).toBeInTheDocument();
  });

  it('handles simulation click', async () => {
    render(<GasCostEstimator />);
    
    const simulateBtn = screen.getByTestId('simulate-btn');
    expect(simulateBtn).toHaveTextContent('Run Simulation');
    
    fireEvent.click(simulateBtn);
    
    expect(simulateBtn).toHaveTextContent('Analyzing...');
    expect(simulateBtn).toBeDisabled();
    
    await waitFor(() => {
      expect(simulateBtn).toHaveTextContent('Run Simulation');
      expect(simulateBtn).not.toBeDisabled();
    }, { timeout: 1000 });
  });

  it('calculates estimated cost correctly', () => {
    render(<GasCostEstimator />);
    
    // Select Token Transfer (Base 1500)
    fireEvent.click(screen.getByTestId('select-token'));
    
    const estimatedElement = screen.getByTestId('estimated-cost');
    const estimatedValue = parseInt(estimatedElement.textContent!.replace(/,/g, ''));
    
    // Initial cost with complexity 1 (1500 * (1 + random(0 to 0.1))) => 1500 to 1650
    expect(estimatedValue).toBeGreaterThanOrEqual(1500);
    expect(estimatedValue).toBeLessThanOrEqual(1650);
  });
});

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ContractAbiExplorer } from './ContractAbiExplorer';
import { describe, it, expect, vi } from 'vitest';

describe('ContractAbiExplorer', () => {
  it('renders default Counter contract and its functions list', () => {
    render(<ContractAbiExplorer />);
    
    expect(screen.getByText('Counter')).toBeInTheDocument();
    expect(screen.getByText('increment')).toBeInTheDocument();
    expect(screen.getByText('decrement')).toBeInTheDocument();
    expect(screen.getByText('get_value')).toBeInTheDocument();
    expect(screen.getByText('reset')).toBeInTheDocument();
  });

  it('switches between contracts', () => {
    render(<ContractAbiExplorer />);
    
    const tokenBtn = screen.getByTestId('abi-select-token');
    fireEvent.click(tokenBtn);

    expect(screen.getByText('initialize')).toBeInTheDocument();
    expect(screen.getByText('balance')).toBeInTheDocument();
    expect(screen.getByText('transfer')).toBeInTheDocument();
  });

  it('renders input fields for selected functions dynamically', () => {
    render(<ContractAbiExplorer />);
    
    // Select reset function on Counter
    const resetBtn = screen.getByTestId('method-reset');
    fireEvent.click(resetBtn);

    expect(screen.getByLabelText('to (u32)')).toBeInTheDocument();
  });

  it('handles execution and displays output stats', async () => {
    render(<ContractAbiExplorer />);
    
    const executeBtn = screen.getByTestId('execute-btn');
    fireEvent.click(executeBtn);

    expect(executeBtn).toHaveTextContent('Executing Simulation...');
    expect(executeBtn).toBeDisabled();

    await waitFor(() => {
      expect(screen.getByTestId('execution-result')).toBeInTheDocument();
      expect(screen.getByText('Simulation Output')).toBeInTheDocument();
      expect(screen.getByText('Gas Expended')).toBeInTheDocument();
    }, { timeout: 1500 });
  });
});

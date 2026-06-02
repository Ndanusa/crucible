import { render, screen, fireEvent } from '@testing-library/react';
import { MultiChainDashboard } from './MultiChainDashboard';
import { describe, it, expect, vi } from 'vitest';

describe('MultiChainDashboard', () => {
  it('renders configured networks list', () => {
    render(<MultiChainDashboard />);
    expect(screen.getAllByText('Soroban Mainnet')[0]).toBeInTheDocument();
    expect(screen.getAllByText('Soroban Testnet')[0]).toBeInTheDocument();
    expect(screen.getAllByText('Soroban Futurenet')[0]).toBeInTheDocument();
  });

  it('allows network selection and displays corresponding details', () => {
    render(<MultiChainDashboard />);
    
    const mainnetCard = screen.getByTestId('network-card-mainnet');
    fireEvent.click(mainnetCard);

    expect(mainnetCard).toHaveClass('active');
    expect(screen.getByTestId('details-rpc-url')).toHaveTextContent('https://soroban-mainnet.stellar.org:443');
    expect(screen.getByTestId('details-passphrase')).toHaveTextContent('Public Global Stellar Network ; October 2015');
  });

  it('shows and hides custom network form', () => {
    render(<MultiChainDashboard />);
    
    const addToggleBtn = screen.getByTestId('add-network-toggle');
    fireEvent.click(addToggleBtn);

    expect(screen.getByTestId('add-network-form')).toBeInTheDocument();

    const cancelBtn = screen.getByText('Cancel');
    fireEvent.click(cancelBtn);

    expect(screen.queryByTestId('add-network-form')).not.toBeInTheDocument();
  });

  it('allows adding and deleting custom networks', () => {
    render(<MultiChainDashboard />);
    
    // Open Form
    fireEvent.click(screen.getByTestId('add-network-toggle'));

    // Input fields
    const nameInput = screen.getByLabelText('Network Name');
    const rpcInput = screen.getByLabelText('RPC Node URL');
    const passInput = screen.getByLabelText('Network Passphrase');

    fireEvent.change(nameInput, { target: { value: 'Private Devnet' } });
    fireEvent.change(rpcInput, { target: { value: 'http://127.0.0.1:9000' } });
    fireEvent.change(passInput, { target: { value: 'My Passphrase' } });

    // Submit
    fireEvent.submit(screen.getByTestId('add-network-form'));

    // Check custom badge / details
    expect(screen.getAllByText('Private Devnet')[0]).toBeInTheDocument();
    expect(screen.getAllByText('Custom')[0]).toBeInTheDocument();

    // Delete custom network
    const deleteBtn = screen.getByTestId(/delete-network-custom-/);
    fireEvent.click(deleteBtn);

    expect(screen.queryByText('Private Devnet')).not.toBeInTheDocument();
  });

  it('handles invalid JSON in localStorage gracefully', () => {
    localStorage.setItem('crucible_custom_networks', '{ invalid_json ');
    
    // Spy on console.error to avoid noisy test output
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    
    render(<MultiChainDashboard />);
    
    // Should still render default networks
    expect(screen.getAllByText('Soroban Mainnet')[0]).toBeInTheDocument();
    expect(consoleSpy).toHaveBeenCalledWith('Failed to parse custom networks', expect.any(Error));
    
    consoleSpy.mockRestore();
    localStorage.removeItem('crucible_custom_networks');
  });
});

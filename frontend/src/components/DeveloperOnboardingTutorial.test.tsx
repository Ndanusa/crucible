import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DeveloperOnboardingTutorial } from './DeveloperOnboardingTutorial';

describe('DeveloperOnboardingTutorial', () => {
  beforeEach(() => {
    render(<DeveloperOnboardingTutorial />);
  });

  it('renders the onboarding tutorial component', () => {
    expect(screen.getByTestId('onboarding-tutorial')).toBeInTheDocument();
    expect(screen.getByText('Developer Onboarding')).toBeInTheDocument();
    expect(screen.getByText('Master the Crucible toolchain in minutes')).toBeInTheDocument();
  });

  it('displays the first step by default', () => {
    const activeStepTitle = screen.getAllByText('Welcome to Crucible');
    expect(activeStepTitle.length).toBeGreaterThan(0);
    expect(screen.getByText(/Crucible is your comprehensive developer portal/i)).toBeInTheDocument();
  });

  it('navigates to the next step when Next is clicked', () => {
    const nextBtn = screen.getByText(/Next/i);
    fireEvent.click(nextBtn);
    
    // Should be on 'Compiling Contracts'
    const activeStepTitle = screen.getAllByText('Compiling Contracts');
    expect(activeStepTitle.length).toBeGreaterThan(0);
    expect(screen.getByText(/The Crucible Compiler Service allows you to compile/i)).toBeInTheDocument();
  });

  it('disables the Previous button on the first step', () => {
    const prevBtn = screen.getByText(/Previous/i);
    expect(prevBtn).toBeDisabled();
  });

  it('enables the Previous button on subsequent steps', () => {
    const nextBtn = screen.getByText(/Next/i);
    fireEvent.click(nextBtn);
    
    const prevBtn = screen.getByText(/Previous/i);
    expect(prevBtn).not.toBeDisabled();
  });

  it('navigates to the previous step when Previous is clicked', () => {
    const nextBtn = screen.getByText(/Next/i);
    fireEvent.click(nextBtn); // Go to step 2
    
    const prevBtn = screen.getByText(/Previous/i);
    fireEvent.click(prevBtn); // Go back to step 1
    
    expect(screen.getByText(/Crucible is your comprehensive developer portal/i)).toBeInTheDocument();
  });

  it('shows Finish Tutorial button on the last step', () => {
    const nextBtn = screen.getByText(/Next/i);
    fireEvent.click(nextBtn); // To step 2
    fireEvent.click(nextBtn); // To step 3
    fireEvent.click(nextBtn); // To step 4 (last)
    
    expect(screen.getByText(/Finish Tutorial/i)).toBeInTheDocument();
  });

  it('allows jumping to a step by clicking the sidebar item', () => {
    const step3 = screen.getByTestId('step-gas');
    fireEvent.click(step3);
    
    expect(screen.getByText(/Deploying and invoking contracts requires fees/i)).toBeInTheDocument();
  });

  it('marks step as completed when clicking Next', () => {
    // Click next to move past step 1
    fireEvent.click(screen.getByText(/Next/i));
    
    // Sidebar item for step 1 should have 'completed' class
    const step1 = screen.getByTestId('step-welcome');
    expect(step1.className).toContain('completed');
  });

  it('marks final step as completed when clicking Finish', () => {
    // Navigate to end
    fireEvent.click(screen.getByText(/Next/i));
    fireEvent.click(screen.getByText(/Next/i));
    fireEvent.click(screen.getByText(/Next/i));
    
    const finishBtn = screen.getByText(/Finish Tutorial/i);
    fireEvent.click(finishBtn);
    
    const step4 = screen.getByTestId('step-complete');
    expect(step4.className).toContain('completed');

    // Click finish again to hit the already-included branch
    fireEvent.click(finishBtn);
    expect(step4.className).toContain('completed');
  });

  it('does not duplicate completed step when next is clicked again', () => {
    const nextBtn = screen.getByText(/Next/i);
    fireEvent.click(nextBtn); // Step 1 completed
    
    const prevBtn = screen.getByText(/Previous/i);
    fireEvent.click(prevBtn); // Go back to Step 1
    
    const nextBtnAgain = screen.getByText(/Next/i);
    fireEvent.click(nextBtnAgain); // Click next on Step 1 again
    
    const step1 = screen.getByTestId('step-welcome');
    expect(step1.className).toContain('completed');
  });
});

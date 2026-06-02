import React, { useState } from 'react';
import { Zap, CheckCircle, ChevronRight, ChevronLeft, Rocket, Terminal } from 'lucide-react';
import './DeveloperOnboardingTutorial.css';

interface Step {
  id: string;
  title: string;
  duration: string;
  icon: React.ReactNode;
  content: React.ReactNode;
}

export const DeveloperOnboardingTutorial: React.FC = () => {
  const [activeStepIndex, setActiveStepIndex] = useState(0);
  const [completedSteps, setCompletedSteps] = useState<string[]>([]);

  const steps: Step[] = [
    {
      id: 'welcome',
      title: 'Welcome to Crucible',
      duration: '2 min',
      icon: <Rocket size={20} />,
      content: (
        <>
          <p>
            Crucible is your comprehensive developer portal for the Soroban ecosystem. 
            Here, you can compile contracts, estimate gas costs, audit dependencies, 
            and interact with the multichain node manager—all from one unified interface.
          </p>
          <div className="interactive-demo">
            <div className="demo-title">What you'll learn in this tutorial:</div>
            <ul style={{ color: '#cbd5e1', paddingLeft: '20px', lineHeight: '1.8' }}>
              <li>Navigating the Crucible workspace</li>
              <li>Compiling your first Soroban smart contract</li>
              <li>Estimating gas costs for deployments</li>
              <li>Auditing project dependencies for vulnerabilities</li>
            </ul>
          </div>
        </>
      )
    },
    {
      id: 'compiler',
      title: 'Compiling Contracts',
      duration: '5 min',
      icon: <Terminal size={20} />,
      content: (
        <>
          <p>
            The Crucible Compiler Service allows you to compile your Rust/Soroban 
            smart contracts directly to WebAssembly format without leaving your browser.
          </p>
          <p>
            To use the compiler, navigate to the <strong>Compiler Service</strong> tab, 
            paste your Soroban source code, and hit "Compile Source". The system will 
            output the build status, WASM size, compilation time, and SHA256 hash.
          </p>
          <div className="interactive-demo">
            <div className="demo-title">Example Contract Code</div>
            <div className="code-block">
{`#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn hello(env: Env, to: Symbol) -> Symbol {
        Symbol::new(&env, &format!("Hello {}", to))
    }
}`}
            </div>
          </div>
        </>
      )
    },
    {
      id: 'gas',
      title: 'Estimating Gas',
      duration: '3 min',
      icon: <Zap size={20} />,
      content: (
        <>
          <p>
            Deploying and invoking contracts requires fees (Gas). The <strong>Gas Estimator</strong> helps you predict these costs before executing transactions on the network.
          </p>
          <p>
            You can input transaction parameters, select the network (Mainnet/Testnet), 
            and see a breakdown of base fees, CPU constraints, and state rent costs.
          </p>
          <div className="interactive-demo">
            <div className="demo-title">Did you know?</div>
            <p style={{ margin: 0, color: '#e2e8f0' }}>
              State rent is a unique feature of Soroban that requires contracts to pay 
              for the storage they consume over time. You can visualize these ongoing 
              costs using our estimator charts.
            </p>
          </div>
        </>
      )
    },
    {
      id: 'complete',
      title: 'Ready to Build',
      duration: '1 min',
      icon: <CheckCircle size={20} />,
      content: (
        <>
          <p>
            You're now ready to start building with Crucible! 
          </p>
          <p>
            Explore the other tabs like the <strong>Dependency Analyzer</strong> to ensure 
            your Cargo.toml doesn't contain vulnerable crates, and the <strong>Node Manager</strong> to check network health.
          </p>
          <div className="interactive-demo" style={{ textAlign: 'center', padding: '40px' }}>
            <Rocket size={48} color="#10b981" style={{ marginBottom: '16px' }} />
            <h3 style={{ color: '#f8fafc', margin: '0 0 8px 0' }}>Tutorial Complete!</h3>
            <p style={{ color: '#94a3b8', margin: 0 }}>Happy coding on Soroban.</p>
          </div>
        </>
      )
    }
  ];

  const handleNext = () => {
    if (!completedSteps.includes(steps[activeStepIndex].id)) {
      setCompletedSteps([...completedSteps, steps[activeStepIndex].id]);
    }
    if (activeStepIndex < steps.length - 1) {
      setActiveStepIndex(activeStepIndex + 1);
    }
  };

  const handlePrev = () => {
    if (activeStepIndex > 0) {
      setActiveStepIndex(activeStepIndex - 1);
    }
  };

  const activeStep = steps[activeStepIndex];

  return (
    <div className="tutorial-container container-panel" data-testid="onboarding-tutorial">
      <div className="tutorial-header">
        <h2>Developer Onboarding</h2>
        <p>Master the Crucible toolchain in minutes</p>
      </div>

      <div className="tutorial-content">
        <div className="tutorial-sidebar">
          {steps.map((step, index) => {
            const isActive = index === activeStepIndex;
            const isCompleted = completedSteps.includes(step.id);
            
            return (
              <div 
                key={step.id} 
                className={`step-item ${isActive ? 'active' : ''} ${isCompleted ? 'completed' : ''}`}
                onClick={() => setActiveStepIndex(index)}
                data-testid={`step-${step.id}`}
              >
                <div className="step-icon-wrapper">
                  {isCompleted && !isActive ? <CheckCircle size={20} /> : step.icon}
                </div>
                <div className="step-info">
                  <span className="step-title">{step.title}</span>
                  <span className="step-duration">{step.duration}</span>
                </div>
              </div>
            );
          })}
        </div>

        <div className="tutorial-main glass-panel active-step-card">
          <div className="active-step-header">
            <div className="active-step-icon">
              {activeStep.icon}
            </div>
            <h3 className="active-step-title">{activeStep.title}</h3>
          </div>
          
          <div className="active-step-content">
            {activeStep.content}
          </div>

          <div className="tutorial-actions">
            <button 
              className="nav-btn btn-prev" 
              onClick={handlePrev}
              disabled={activeStepIndex === 0}
            >
              <ChevronLeft size={18} />
              Previous
            </button>
            
            {activeStepIndex === steps.length - 1 ? (
              <button 
                className="nav-btn btn-complete"
                onClick={() => {
                  if (!completedSteps.includes(activeStep.id)) {
                    setCompletedSteps([...completedSteps, activeStep.id]);
                  }
                }}
              >
                <CheckCircle size={18} />
                Finish Tutorial
              </button>
            ) : (
              <button 
                className="nav-btn btn-next" 
                onClick={handleNext}
              >
                Next
                <ChevronRight size={18} />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

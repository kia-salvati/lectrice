import { Component } from 'react';
import centered from '../../styles/centered.module.css';
import controls from '../../styles/controls.module.css';

interface ErrorBoundaryProps { children: React.ReactNode; }
interface ErrorBoundaryState { crashed: boolean; }

/** The last resort for a render that throws: a way to reload instead of an empty window. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { crashed: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { crashed: true };
  }

  componentDidCatch(error: unknown): void {
    console.error('UI crashed', error);
  }

  render(): React.ReactNode {
    if (!this.state.crashed) return this.props.children;
    return (
      <div className={centered.centered}>
        <h1 className={centered.title}>Something went wrong</h1>
        <p className={centered.text}>Lectrice hit an unexpected error.</p>
        <div className={centered.actions}>
          <button type="button" className={`${controls.button} ${controls.primary}`} onClick={() => window.location.reload()}>Reload</button>
        </div>
      </div>
    );
  }
}

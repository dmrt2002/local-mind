import React, { Component, ErrorInfo, ReactNode } from 'react';
import { logger } from '../utils/logger';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
    };
  }

  static getDerivedStateFromError(error: Error): State {
    return {
      hasError: true,
      error,
      errorInfo: null,
    };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    logger.error('ErrorBoundary caught an error:', error);
    logger.error('Error info:', errorInfo);
    this.setState({
      error,
      errorInfo,
    });
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div style={{
          padding: '50px',
          textAlign: 'center',
          background: '#fff',
          minHeight: '100vh',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
        }}>
          <h1 style={{ color: '#dc2626', marginBottom: '20px' }}>
            ⚠️ Something went wrong
          </h1>
          <div style={{
            background: '#fef2f2',
            border: '1px solid #fecaca',
            borderRadius: '8px',
            padding: '20px',
            maxWidth: '600px',
            textAlign: 'left',
            marginBottom: '20px',
          }}>
            <h2 style={{ fontSize: '18px', marginBottom: '10px' }}>
              Error Details:
            </h2>
            <pre style={{
              background: '#fff',
              padding: '10px',
              borderRadius: '4px',
              overflow: 'auto',
              fontSize: '12px',
              color: '#991b1b',
            }}>
              {this.state.error?.message || 'Unknown error'}
            </pre>
            {this.state.errorInfo && (
              <details style={{ marginTop: '10px' }}>
                <summary style={{ cursor: 'pointer', color: '#991b1b' }}>
                  Stack Trace
                </summary>
                <pre style={{
                  background: '#fff',
                  padding: '10px',
                  borderRadius: '4px',
                  overflow: 'auto',
                  fontSize: '11px',
                  marginTop: '10px',
                  maxHeight: '300px',
                }}>
                  {this.state.errorInfo.componentStack}
                </pre>
              </details>
            )}
          </div>
          <button
            onClick={() => {
              this.setState({ hasError: false, error: null, errorInfo: null });
              window.location.reload();
            }}
            style={{
              padding: '12px 24px',
              background: '#3b82f6',
              color: 'white',
              border: 'none',
              borderRadius: '8px',
              cursor: 'pointer',
              fontSize: '16px',
              fontWeight: '600',
            }}
          >
            Reload Application
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;


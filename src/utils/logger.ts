import log from 'loglevel';

// Configure log level based on environment
if (import.meta.env.DEV) {
  log.setLevel('DEBUG'); // Show all logs in development
} else {
  log.setLevel('WARN'); // Only warnings and errors in production
}

// Create a React-specific logger with better formatting
export const logger = {
  debug: (...args: any[]) => {
    if (import.meta.env.DEV) {
      log.debug('[DEBUG]', ...args);
    }
  },
  info: (...args: any[]) => {
    log.info('[INFO]', ...args);
  },
  warn: (...args: any[]) => {
    log.warn('[WARN]', ...args);
  },
  error: (...args: any[]) => {
    log.error('[ERROR]', ...args);
  },
  trace: (component: string, action: string, ...args: any[]) => {
    if (import.meta.env.DEV) {
      log.debug(`[${component}] ${action}`, ...args);
    }
  },
};

// React component lifecycle logger
export const logComponent = (componentName: string) => {
  return {
    mount: (...args: any[]) => logger.trace(componentName, 'MOUNT', ...args),
    render: (...args: any[]) => logger.trace(componentName, 'RENDER', ...args),
    update: (...args: any[]) => logger.trace(componentName, 'UPDATE', ...args),
    unmount: (...args: any[]) => logger.trace(componentName, 'UNMOUNT', ...args),
    effect: (effectName: string, ...args: any[]) => 
      logger.trace(componentName, `EFFECT:${effectName}`, ...args),
    error: (...args: any[]) => logger.error(`[${componentName}]`, ...args),
  };
};

export default logger;


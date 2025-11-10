import { useEffect, useRef } from 'react';
import { logComponent } from '../utils/logger';

/**
 * Hook to add automatic logging to React components
 * Usage: const log = useLogger('ComponentName');
 */
export function useLogger(componentName: string) {
  const logger = useRef(logComponent(componentName));
  const renderCount = useRef(0);

  useEffect(() => {
    logger.current.mount('Component mounted');
    renderCount.current++;

    return () => {
      logger.current.unmount('Component unmounted');
    };
  }, []);

  useEffect(() => {
    renderCount.current++;
    logger.current.render(`Render #${renderCount.current}`);
  });

  return logger.current;
}


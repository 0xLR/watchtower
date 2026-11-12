import { createContext } from 'react';
import type { MediaEngine } from '../../../../../packages/ipc-types/MediaEngine';

export const EngineAvailability = createContext<MediaEngine[]>(['retina']);

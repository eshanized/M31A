import React from 'react';
import { AppProvider } from '../stores/AppContext';
import { Layout } from './Layout';
import '../styles/theme.css';

export const App: React.FC = () => {
  return (
    <AppProvider>
      <Layout />
    </AppProvider>
  );
};

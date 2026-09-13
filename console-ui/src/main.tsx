import React from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { BrokerApp } from './BrokerApp';
import './styles.css';

const params = new URLSearchParams(window.location.search);
createRoot(document.getElementById('root')!).render(<React.StrictMode>{params.has('managed') && !params.has('desktop') ? <BrokerApp /> : <App />}</React.StrictMode>);

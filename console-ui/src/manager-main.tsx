import React from 'react';
import {createRoot} from 'react-dom/client';
import {ManagerApp} from './ManagerApp';
import {BrokerApp} from './BrokerApp';
import {App} from './App';
import './styles.css';
const params = new URLSearchParams(location.search);
createRoot(document.getElementById('root')!).render(<React.StrictMode>{params.has('device') ? params.has('desktop') ? <App/> : <BrokerApp/> : <ManagerApp/>}</React.StrictMode>);

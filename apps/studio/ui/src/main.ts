// Entry point: loads the styles and starts the app.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import './styles/tokens.css';
import './styles/app.css';
import { startApp } from './app';

const root = document.getElementById('app');
if (root) void startApp(root);

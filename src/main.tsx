import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './app/App'
import { installFocusModality } from './shared/ui/focus-modality'
import './index.css'

const removeFocusModality = installFocusModality()
if (import.meta.hot) import.meta.hot.dispose(removeFocusModality)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
// UX-101 : typographie auto-hébergée (woff2 subset latin, aucune requête réseau externe).
import '@fontsource/outfit/latin-400.css'
import '@fontsource/outfit/latin-600.css'
import '@fontsource/space-grotesk/latin-500.css'
import '@fontsource/space-grotesk/latin-700.css'
import '@fontsource/jetbrains-mono/latin-400.css'
import '@fontsource/jetbrains-mono/latin-500.css'
import './index.css'
import App from './App.tsx'
import { ErrorBoundary } from './components/ErrorBoundary'

const root = document.getElementById('root')!

const params = new URLSearchParams(window.location.search)

if (params.get('spike') === '3d') {
  // Banc d'essai 3D (three.js pur) chargé à la volée.
  import('./render3d/spike/Spike3D').then(({ default: Spike3D }) => {
    createRoot(root).render(
      <StrictMode>
        <Spike3D />
      </StrictMode>,
    )
  })
} else if (params.get('scene') === '3d') {
  // Tranche verticale : scène 3D pilotée par le moteur Rust (RenderPacket).
  import('./render3d/Scene3DView').then(({ default: Scene3DView }) => {
    createRoot(root).render(
      <StrictMode>
        <Scene3DView />
      </StrictMode>,
    )
  })
} else {
  createRoot(root).render(
    <StrictMode>
      <ErrorBoundary label="Studio">
        <App />
      </ErrorBoundary>
    </StrictMode>,
  )
}

import './styles/main.css'
import '@fontsource/geist-sans/400.css'
import '@fontsource/geist-sans/500.css'
import '@fontsource/geist-sans/600.css'
import '@fontsource/geist-sans/700.css'
import '@fontsource-variable/geist-mono'
import 'remixicon/fonts/remixicon.css'
import App from './App.svelte'

const app = new App({
  target: document.getElementById('app'),
})

export default app

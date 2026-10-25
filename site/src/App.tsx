import { Agents } from './components/Agents';
import { Feedback } from './components/Feedback';
import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';

export function App() {
  return (
    <>
      <Nav />
      <main id="top">
        <Hero />
        <Feedback />
        <Agents />
      </main>
      <Footer />
    </>
  );
}

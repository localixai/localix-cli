import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';
import { Callers, Pipeline, Small } from './components/Callers';
import { Window } from './components/Window';

export function App() {
  return (
    <>
      <Nav />
      <main id="top">
        <Hero />
        <Callers />
        <Window />
        <Pipeline />
        <Small />
      </main>
      <Footer />
    </>
  );
}

import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';
import { Blocks, Own, Pipeline } from './components/Sections';
import { Window } from './components/Window';

export function App() {
  return (
    <>
      <Nav />
      <main id="main">
        <Hero />
        <Blocks />
        <Window />
        <Pipeline />
        <Own />
      </main>
      <Footer />
    </>
  );
}

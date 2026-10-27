import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';
import { Blocks, Own, Pipeline } from './components/Sections';

export function App() {
  return (
    <>
      <Nav />
      <main id="main">
        <Hero />
        <Blocks />
        <Pipeline />
        <Own />
      </main>
      <Footer />
    </>
  );
}

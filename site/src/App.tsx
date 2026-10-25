import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';
import { Window } from './components/Window';

export function App() {
  return (
    <>
      <Nav />
      <main id="top">
        <Hero />
        <Window />
      </main>
      <Footer />
    </>
  );
}

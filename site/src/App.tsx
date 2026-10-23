import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Nav } from './components/Nav';

export function App() {
  return (
    <>
      <Nav />
      <main id="top">
        <Hero />
      </main>
      <Footer />
    </>
  );
}

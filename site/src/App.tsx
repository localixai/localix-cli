import { Ci } from './components/Ci';
import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { Loop } from './components/Loop';
import { Nav } from './components/Nav';
import { Uses } from './components/Uses';

export function App() {
  return (
    <>
      <Nav />
      <main id="main">
        <Hero />
        <Ci />
        <Uses />
        <Loop />
      </main>
      <Footer />
    </>
  );
}

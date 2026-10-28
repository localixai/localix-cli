export type Phone = {
  id: string;
  brand: 'Apple' | 'Samsung' | 'Google' | 'Xiaomi';
  name: string;
  storage: string;
  price: number; // euros, VAT included
  stock: number;
};

export const PHONES: Phone[] = [
  { id: 'iphone-17-pro', brand: 'Apple', name: 'iPhone 17 Pro', storage: '256 GB', price: 1299, stock: 4 },
  { id: 'iphone-17', brand: 'Apple', name: 'iPhone 17', storage: '128 GB', price: 949, stock: 11 },
  { id: 'galaxy-s26-ultra', brand: 'Samsung', name: 'Galaxy S26 Ultra', storage: '512 GB', price: 1449, stock: 2 },
  { id: 'galaxy-a57', brand: 'Samsung', name: 'Galaxy A57', storage: '128 GB', price: 429, stock: 0 },
  { id: 'pixel-11', brand: 'Google', name: 'Pixel 11', storage: '128 GB', price: 799, stock: 7 },
  { id: 'redmi-note-15', brand: 'Xiaomi', name: 'Redmi Note 15', storage: '256 GB', price: 289, stock: 15 },
];

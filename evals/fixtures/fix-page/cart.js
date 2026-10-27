// The cart: the prices added, and their total.
const cart = [];

function add(button) {
  cart.push(button.dataset.price);
  render();
}

function total(prices) {
  return prices.reduce((sum, price) => sum + price, 0);
}

function render() {
  if (typeof document === "undefined") return;
  document.getElementById("count").textContent = cart.length;
  document.getElementById("total").textContent = "$" + total(cart);
}

if (typeof module !== "undefined") module.exports = { add, total, cart };

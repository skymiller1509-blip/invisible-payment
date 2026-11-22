/* Network and template settings for the on-chain (live) path.
   Leave templateAddress empty until the template is published.
   The simulation does not use this file.
   Publish: upload invisible_payment.wasm from the Sapient Home page
   (Publish Template, Estimate Fee first) on Esmeralda, then paste the
   resulting template_... address below. */
window.INVISIBLE_PAYMENT_CONFIG = {
  // "esmeralda" (testnet) or "mainnet"
  network: "esmeralda",
  // Paste the published address, e.g. "template_abc123...". Empty = live
  // mode can connect and read the epoch, but cannot lock funds on-chain.
  templateAddress: "",
  indexer: {
    // Verified live 2026-11-19: GET /network returns
    // {"network":"esmeralda","network_byte":38,"epoch":13233}.
    esmeralda: "https://ootle-indexer-a.tari.com",
    mainnet: "https://indexer-a.ootle.tari.com",
  },
  explorer: {
    // Veil, the Ootle L2 explorer. testnet-explorer.tari.mw talks to the
    // Esmeralda indexer; explorer.tari.mw talks to mainnet.
    esmeralda: "https://testnet-explorer.tari.mw",
    mainnet: "https://explorer.tari.mw",
  },
};

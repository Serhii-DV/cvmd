const { resolve } = require("node:path");

function container(fence, name) {
  const pattern = new RegExp(
    `^${fence}[ \\t]+${name}[ \\t]*\\r?\\n([\\s\\S]*?)^${fence}[ \\t]*(?:\\r?\\n|$)`,
    "m"
  );

  return {
    name,
    level: "block",

    start(src) {
      const opening = new RegExp(
        `^${fence}[ \\t]+${name}[ \\t]*$`,
        "m"
      );
      const index = src.search(opening);
      return index === -1 ? undefined : index;
    },

    tokenizer(src) {
      const match = pattern.exec(src);
      if (!match || match.index !== 0) return;

      return {
        type: name,
        raw: match[0],
        tokens: this.lexer.blockTokens(match[1], []),
      };
    },

    renderer(token) {
      return `<div class="${name}">
        ${this.parser.parse(token.tokens)}
      </div>\n`;
    },
  };
}

module.exports = {
  stylesheet: [resolve(__dirname, "cv.css"), resolve(__dirname, "columns.css")],

  pdf_options: {
    format: "A4",
    margin: "10mm",
    printBackground: true,
  },

  marked_extensions: [
    {
      extensions: [
        container("::::", "columns"),
        container(":::", "left-column"),
        container(":::", "right-column"),
      ],
    },
  ],
};

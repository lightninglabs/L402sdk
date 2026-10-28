import assert from 'node:assert/strict';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ReactMarkdown from 'react-markdown';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';

const render = text => renderToStaticMarkup(React.createElement(
  ReactMarkdown, { remarkPlugins: [remarkMath], rehypePlugins: [rehypeKatex] }, text,
));
assert.match(render('$x^2 + 1$'), /katex/);
assert.doesNotMatch(render('$\\href{javascript:alert(1)}{click}$'), /href="javascript:/);
console.log('KaTeX rendering and untrusted-link checks passed');

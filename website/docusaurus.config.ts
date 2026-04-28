import * as fs from 'node:fs';
import * as path from 'node:path';
import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const cargoToml = fs.readFileSync(
  path.resolve(__dirname, '../Cargo.toml'),
  'utf8',
);
const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)![1];

const config: Config = {
  title: 'pg_grpc',
  tagline: 'Make gRPC calls directly from PostgreSQL',
  favicon: 'img/favicon.svg',

  future: {
    v4: true,
  },

  url: 'https://csenshi.github.io',
  baseUrl: '/pg_grpc/',

  organizationName: 'CSenshi',
  projectName: 'pg_grpc',
  trailingSlash: false,

  onBrokenLinks: 'throw',

  customFields: {version},

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  markdown: {
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },
  themes: ['@docusaurus/theme-mermaid'],

  stylesheets: [
    'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600;700&display=swap',
  ],

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: '/',
          editUrl: 'https://github.com/CSenshi/pg_grpc/tree/main/website/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  plugins: [
    [
      '@easyops-cn/docusaurus-search-local',
      {
        hashed: true,
        indexBlog: false,
        docsRouteBasePath: '/',
      },
    ],
    // Copy raw .md files to the build output so agents can fetch them at *.md URLs
    function copyMarkdownForAgents() {
      return {
        name: 'copy-markdown-for-agents',
        async postBuild({outDir}: {outDir: string}) {
          const docsDir = path.resolve(__dirname, 'docs');
          async function copyMdFiles(srcDir: string, destDir: string) {
            await fs.promises.mkdir(destDir, {recursive: true});
            const entries = await fs.promises.readdir(srcDir, {withFileTypes: true});
            for (const entry of entries) {
              const srcPath = path.join(srcDir, entry.name);
              if (entry.isDirectory()) {
                await copyMdFiles(srcPath, path.join(destDir, entry.name));
              } else if (entry.name.endsWith('.md') || entry.name.endsWith('.mdx')) {
                const destName = entry.name.replace(/\.mdx$/, '.md');
                await fs.promises.copyFile(srcPath, path.join(destDir, destName));
              }
            }
          }
          await copyMdFiles(docsDir, outDir);
        },
      };
    },
  ],

  themeConfig: {
    image: 'img/og.png',
    metadata: [
      {
        name: 'description',
        content:
          'PostgreSQL extension to call gRPC services directly from SQL',
      },
      {
        name: 'keywords',
        content:
          'postgresql, postgres, grpc, extension, rust, pgrx, protobuf, sql, tonic, pg',
      },
      {property: 'og:type', content: 'website'},
      {property: 'og:site_name', content: 'pg_grpc'},
      {property: 'og:image:width', content: '900'},
      {property: 'og:image:height', content: '450'},
      {
        property: 'og:image:alt',
        content: 'pg_grpc — Make gRPC calls directly from PostgreSQL',
      },
      {name: 'twitter:card', content: 'summary_large_image'},
      {
        name: 'twitter:image:alt',
        content: 'pg_grpc — Make gRPC calls directly from PostgreSQL',
      },
    ],
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: 'pg_grpc',
      logo: {
        alt: '',
        src: 'img/logo.svg',
      },
      items: [
        {to: '/introduction', label: 'Docs', position: 'left'},
        {to: '/quickstart', label: 'Quickstart', position: 'left'},
        {to: '/reference', label: 'API', position: 'left'},
        {
          type: 'html',
          position: 'right',
          value: `<span class="alpha-pill"><span class="alpha-dot"></span>v${version} · alpha</span>`,
        },
        {
          type: 'html',
          position: 'right',
          value:
            '<a class="gh-btn" href="https://github.com/CSenshi/pg_grpc" target="_blank" rel="noopener noreferrer" aria-label="GitHub"><svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8a8 8 0 0 0 5.47 7.59c.4.07.55-.17.55-.38v-1.34c-2.23.49-2.7-1.07-2.7-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.05-.49.05-.49.81.06 1.23.83 1.23.83.72 1.23 1.88.87 2.34.67.07-.52.28-.87.51-1.07-1.78-.2-3.65-.89-3.65-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.13 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.93.08 2.13.51.56.82 1.27.82 2.15 0 3.07-1.87 3.74-3.65 3.94.29.25.54.73.54 1.48v2.2c0 .21.15.46.55.38A8 8 0 0 0 16 8c0-4.42-3.58-8-8-8z"></path></svg>GitHub</a>',
        },
      ],
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['sql', 'protobuf', 'docker', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;

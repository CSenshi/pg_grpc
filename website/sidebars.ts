import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'introduction',
    'installation',
    'quickstart',
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      items: [
        'guides/tls-and-mtls',
        'guides/user-supplied-protos',
        'guides/large-messages',
        'guides/recipes',
      ],
    },
    'reference',
  ],
};

export default sidebars;

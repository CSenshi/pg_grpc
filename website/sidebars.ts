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
        'guides/user-supplied-protos',
        'guides/async-calls',
        'guides/tls-and-mtls',
        'guides/large-messages',
      ],
    },
    'reference',
  ],
};

export default sidebars;

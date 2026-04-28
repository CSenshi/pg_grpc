import React from 'react';
import Layout from '@theme/Layout';
import Hero from '@site/src/components/Hero/Hero';

export default function Home(): JSX.Element {
  return (
    <Layout
      title="pg_grpc - gRPC calls from PostgreSQL SQL"
      description="A PostgreSQL extension that lets you invoke unary gRPC methods directly from a SQL query. No sidecar, no codegen, no application layer in the middle.">
      <p className="agent-index-hint">For the complete documentation index, see <a href="/pg_grpc/llms.txt">llms.txt</a></p>
      <Hero />
    </Layout>
  );
}

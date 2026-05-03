import React, {useEffect, useRef, useState} from 'react';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import styles from './Hero.module.css';

type TabKey = 't1' | 't2' | 't3';

const TABS: {key: TabKey; label: string}[] = [
  {key: 't1', label: 'reflection'},
  {key: 't2', label: 'staged proto'},
  {key: 't3', label: 'async'},
];

export default function Hero(): JSX.Element {
  const {siteConfig} = useDocusaurusContext();
  const version = siteConfig.customFields?.version as string;
  const [tab, setTab] = useState<TabKey>('t1');
  const userInteractedRef = useRef(false);

  useEffect(() => {
    const id = window.setInterval(() => {
      if (userInteractedRef.current) return;
      setTab((t) => (t === 't1' ? 't2' : t === 't2' ? 't3' : 't1'));
    }, 7000);
    return () => window.clearInterval(id);
  }, []);

  const handleTab = (key: TabKey) => {
    userInteractedRef.current = true;
    setTab(key);
  };

  return (
    <section className={styles.root}>
      <div className={styles.heroBg} />
      <div className={styles.gridOverlay} />

      <div className={`${styles.wrap} ${styles.heroGrid}`}>
        <div>
          <span className={styles.eyebrow}>
            <b>postgres extension</b>
            <span className={styles.sep}>·</span>
            Postgres 13+
            <span className={styles.sep}>·</span>
            v{version}
          </span>

          <h1 className={styles.headline}>
            Call <span className={styles.grpcWord}>gRPC</span> services
            <br />
            directly from <em>SQL</em>
          </h1>

          <p className={styles.lede}>
            <code>pg_grpc</code> is a PostgreSQL extension that lets you invoke
            unary gRPC methods from inside a SQL query - no sidecar, no codegen,
            no application layer in the middle.
          </p>

          <div className={styles.ctaRow}>
            <Link
              className={`${styles.btn} ${styles.btnPrimary}`}
              to="/quickstart">
              Get started
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.4"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true">
                <path d="M5 12h14" />
                <path d="m13 5 7 7-7 7" />
              </svg>
            </Link>
          </div>
        </div>

        <div className={styles.terminal} aria-label="Demo terminal">
          <div className={styles.termBar}>
            <div className={styles.termDots}>
              <span />
              <span />
              <span />
            </div>
            <div className={styles.termTitle}>
              psql - <span className={styles.pgConn}>postgres@db</span>:5432
            </div>
          </div>

          <div className={styles.termTabs} role="tablist">
            {TABS.map((t) => (
              <button
                key={t.key}
                type="button"
                role="tab"
                aria-selected={tab === t.key}
                className={`${styles.termTab} ${
                  tab === t.key ? styles.termTabActive : ''
                }`}
                onClick={() => handleTab(t.key)}>
                {t.label}
              </button>
            ))}
          </div>

          <div className={styles.termBody}>
            {tab === 't1' ? <PaneReflect /> : tab === 't2' ? <PaneStaged /> : <PaneAsync />}
          </div>
        </div>
      </div>
    </section>
  );
}

function PaneAsync(): JSX.Element {
  return (
    <pre>
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tComment}>-- Enqueue without blocking.</span>
      {'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tKw}>SELECT</span>{' '}
      <span className={styles.tFn}>grpc_call_async</span>({'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     '}
      <span className={styles.tStr}>'api.internal:9090'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     '}
      <span className={styles.tStr}>'notify.Notifier/Send'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     '}
      <span className={styles.tStr}>{`'{"user_id": 42}'`}</span>::
      <span className={styles.tKw}>jsonb</span>{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span> );{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tMeta}>{'  grpc_call_async  '}</span>
      {'\n'}
      <span className={styles.tMeta}>{'-------------------'}</span>
      {'\n'}
      <span className={styles.tRow}>{'                 1'}</span>
      {'\n'}
      <span className={styles.tMeta}>(1 row)</span>
      {'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tComment}>-- Fetch the result.</span>
      {'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tKw}>SELECT</span> status, response{' '}
      <span className={styles.tKw}>FROM</span>{' '}
      <span className={styles.tFn}>grpc_call_result</span>(1);{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tMeta}>{'  status  |         response         '}</span>
      {'\n'}
      <span className={styles.tMeta}>{'----------+--------------------------'}</span>
      {'\n'}
      <span className={styles.tRow}>
        {' SUCCESS  | {'}
        <span className={styles.tStr}>"ok"</span>
        {': '}
        <span className={styles.tStr}>true</span>
        {', '}
        <span className={styles.tStr}>"msg_id"</span>
        {': 7}'}
      </span>
      {'\n'}
      <span className={styles.tMeta}>(1 row)</span>
      {'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tCursor} />
    </pre>
  );
}

function PaneStaged(): JSX.Element {
  return (
    <pre>
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tComment}>
        -- No reflection? Use server protos.
      </span>
      {'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tKw}>SELECT</span>{' '}
      <span className={styles.tFn}>grpc_proto_register</span>({'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     '}
      <span className={styles.tStr}>'echo'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     '}
      <span className={styles.tFn}>pg_read_binary_file</span>(
      <span className={styles.tStr}>'echo.fds'</span>){'\n'}
      <span className={styles.tCont}>pg_grpc-#</span> );{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tKw}>SELECT</span>{' '}
      <span className={styles.tFn}>grpc_call</span>({'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     host    '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>'localhost:50051'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     proto   '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>'echo'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     method  '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>'echo.Echoer/Say'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     payload '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>{`'{"message":"hi"}'`}</span>::
      <span className={styles.tKw}>jsonb</span>{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span> );{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tMeta}>{'          grpc_call          '}</span>
      {'\n'}
      <span className={styles.tMeta}>
        {'-----------------------------'}
      </span>
      {'\n'}
      <span className={styles.tRow}>
        {' {'}
        <span className={styles.tStr}>"message"</span>
        {': '}
        <span className={styles.tStr}>"hi"</span>
        {'}'}
      </span>
      {'\n'}
      <span className={styles.tMeta}>(1 row)</span>
      {'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tCursor} />
    </pre>
  );
}

function PaneReflect(): JSX.Element {
  return (
    <pre>
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tKw}>SELECT</span>{' '}
      <span className={styles.tFn}>grpc_call</span>({'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     host    '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>'api.internal:9090'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     method  '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>'users.UserService/GetUser'</span>,{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span>{'     payload '}
      <span className={styles.tArrow}>{'=>'}</span>{' '}
      <span className={styles.tStr}>{`'{"user_id": 42}'`}</span>::
      <span className={styles.tKw}>jsonb</span>{'\n'}
      <span className={styles.tCont}>pg_grpc-#</span> );{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tMeta}>
        {'              grpc_call              '}
      </span>
      {'\n'}
      <span className={styles.tMeta}>
        {'---------------------------------------------'}
      </span>
      {'\n'}
      <span className={styles.tRow}>{' {'}</span>{'\n'}
      <span className={styles.tRow}>
        {'   '}
        <span className={styles.tStr}>"id"</span>
        {': 42,'}
      </span>
      {'\n'}
      <span className={styles.tRow}>
        {'   '}
        <span className={styles.tStr}>"name"</span>
        {': '}
        <span className={styles.tStr}>"Ada Lovelace"</span>
        {','}
      </span>
      {'\n'}
      <span className={styles.tRow}>
        {'   '}
        <span className={styles.tStr}>"email"</span>
        {': '}
        <span className={styles.tStr}>"ada@analytical.engine"</span>
      </span>
      {'\n'}
      <span className={styles.tRow}>{' }'}</span>{'\n'}
      <span className={styles.tMeta}>(1 row)</span>{'\n'}
      <span className={styles.blank} />{'\n'}
      <span className={styles.tPrompt}>pg_grpc=#</span>{' '}
      <span className={styles.tCursor} />
    </pre>
  );
}

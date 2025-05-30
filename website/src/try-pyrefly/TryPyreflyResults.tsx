/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import * as React from 'react';
import * as stylex from '@stylexjs/stylex';
import useBaseUrl from '@docusaurus/useBaseUrl';

export interface PyreflyErrorMessage {
    startLineNumber: number;
    startColumn: number;
    endLineNumber: number;
    endColumn: number;
    message: string;
    kind: string;
    severity: number;
}

export type GoToDefFromError = (
    startLineNumber: number,
    startColumn: number,
    endLineNumber: number,
    endColumn: number
) => void;

interface ErrorMessageProps {
    error: PyreflyErrorMessage;
    goToDef: GoToDefFromError;
}

interface TryPyreflyResultsProps {
    loading: boolean;
    goToDef: GoToDefFromError;
    errors?: ReadonlyArray<PyreflyErrorMessage> | null;
    internalError: string;
    pythonOutput: string;
    isRunning: boolean;
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

function ErrorMessage({
    error,
    goToDef,
}: ErrorMessageProps): React.ReactElement {
    // This logic is meant to be an exact match of how we output errors in the cli defined here:
    // - https://fburl.com/code/e9lqk0h2
    // - https://fburl.com/code/hwhe60zt
    // TODO (T217247871): expose full error message from Pyrefly binary and use it directly here instead of duplicating the logic
    const { startLineNumber, startColumn, endLineNumber, endColumn } = error;

    let rangeStr: string;
    if (startLineNumber === endLineNumber) {
        if (startColumn === endColumn) {
            rangeStr = `${startLineNumber}:${startColumn}`;
        } else {
            rangeStr = `${startLineNumber}:${startColumn}-${endColumn}`;
        }
    } else {
        rangeStr = `${startLineNumber}:${startColumn}-${endLineNumber}:${endColumn}`;
    }

    const errorKindUrl = (
        <a href={useBaseUrl(`en/docs/error-kinds/#${error.kind}`)}>
            {error.kind}
        </a>
    );
    const message = `${rangeStr}: ${error.message} `;

    return (
        <span
            {...stylex.props(styles.msgType)}
            onClick={() =>
                goToDef(startLineNumber, startColumn, endLineNumber, endColumn)
            }
        >
            <span {...stylex.props(styles.errorMessageError)}>ERROR </span>
            {message}
            {'['}
            {errorKindUrl}
            {']'}
        </span>
    );
}

function LoadingAnimation(): React.ReactElement {
    return (
        <div>
            <div {...stylex.props(styles.loader)}>
                <div {...stylex.props(styles.loaderDot, styles.bounce1)}></div>
                <div {...stylex.props(styles.loaderDot, styles.bounce2)}></div>
                <div {...stylex.props(styles.loaderDot)}></div>
            </div>
        </div>
    );
}
export default function TryPyreflyResults({
    loading,
    goToDef,
    errors,
    internalError,
    pythonOutput = '',
    isRunning = false,
    activeTab = 'errors',
    setActiveTab = () => {},
}: TryPyreflyResultsProps): React.ReactElement {
    const activeToolbarTab = activeTab;

    const hasTypecheckErrors =
        errors !== undefined && errors !== null && errors.length > 0;

    return (
        <div
            id="tryPyrefly-results-container"
            {...stylex.props(styles.resultsContainer)}
        >
            <div {...stylex.props(styles.resultsToolbar)}>
                <ul {...stylex.props(styles.tabs, styles.tabsEnabled)}>
                    <li
                        {...stylex.props(
                            styles.tab,
                            activeToolbarTab === 'errors' && styles.selectedTab
                        )}
                        onClick={() => setActiveTab('errors')}
                    >
                        Errors
                    </li>
                    <li
                        {...stylex.props(
                            styles.tab,
                            activeToolbarTab === 'output' && styles.selectedTab
                        )}
                        onClick={() => setActiveTab('output')}
                    >
                        Output
                    </li>
                </ul>
                {/* TODO (T217536145): Add JSON tab to sandbox */}
            </div>
            <div {...stylex.props(styles.results)}>
                {loading && LoadingAnimation()}
                {!loading && activeToolbarTab === 'errors' && (
                    <pre {...stylex.props(styles.resultBody)}>
                        <ul {...stylex.props(styles.errorsList)}>
                            {hasTypecheckErrors ? (
                                errors.map((error, i) => (
                                    <li
                                        key={i}
                                        {...stylex.props(
                                            i > 0 && styles.errorItemSibling
                                        )}
                                    >
                                        <ErrorMessage
                                            key={i}
                                            error={error}
                                            goToDef={goToDef}
                                        />
                                    </li>
                                ))
                            ) : (
                                <li>
                                    {internalError
                                        ? `Pyrefly encountered an internal error: ${internalError}.`
                                        : errors === undefined ||
                                          errors === null
                                        ? 'Pyrefly failed to fetch errors.'
                                        : errors?.length === 0
                                        ? 'No errors!'
                                        : null}
                                </li>
                            )}
                        </ul>
                    </pre>
                )}
                {activeToolbarTab === 'output' && (
                    <pre {...stylex.props(styles.resultBody)}>
                        {isRunning ? (
                            <div>Running...</div>
                        ) : (
                            <div>
                                {pythonOutput.trimStart() ||
                                    'No output, please press the ▶️ Run button.'}
                            </div>
                        )}
                    </pre>
                )}
                {/* TODO (T217536145): Add JSON tab to sandbox */}
            </div>
        </div>
    );
}

// Define keyframes for animations
const skBounceDelayKeyframes = stylex.keyframes({
    '0%, 80%, 100%': { transform: 'scale(0)' },
    '40%': { transform: 'scale(1)' },
});

// Styles for TryPyreflyResults component
const styles = stylex.create({
    resultsContainer: {
        height: 'calc(25vh - var(--ifm-navbar-height) / 4)', // 25% of screen height - nav bar
        position: 'relative',
        fontSize: '12px',
        background: '#f7f7f7',
        borderLeft: '1px solid #ddd',
    },
    resultsToolbar: {
        display: 'flex',
        background: '#fff',
        borderBottom: '1px solid #ddd',
        fontSize: '14px',
    },
    results: {
        overflow: 'auto',
        height: '80%',
    },
    resultBody: {
        padding: '7px 10px',
        marginBottom: 0,
        display: 'flex',
    },
    tabs: {
        display: 'flex',
        listStyle: 'none',
        margin: 0,
        padding: 0,
        pointerEvents: 'none',
    },
    tabsEnabled: {
        pointerEvents: 'auto',
    },
    tab: {
        borderRight: '1px solid #ddd',
        cursor: 'pointer',
        fontWeight: 'bold',
        padding: '7px 15px',
    },
    selectedTab: {
        background: 'white',
        borderBottom: '2px solid #404040',
        marginBottom: '-1px', // cover up container bottom border
    },
    loader: {
        display: 'flex',
        justifyContent: 'center',
        marginTop: '10px',
    },
    loaderDot: {
        width: '14px',
        height: '14px',
        backgroundColor: '#ccc',
        borderRadius: '100%',
        animationName: skBounceDelayKeyframes,
        animationDuration: '1.4s',
        animationIterationCount: 'infinite',
        animationTimingFunction: 'ease-in-out',
        animationFillMode: 'both',
    },
    bounce1: {
        animationDelay: '-320ms',
    },
    bounce2: {
        animationDelay: '-160ms',
    },
    errorsList: {
        listStyle: 'none',
        margin: 0,
        padding: 0,
    },
    errorItemSibling: {
        marginTop: '10px',
        paddingTop: '10px',
        borderTop: 'solid #eee 1px',
    },
    errorNestedItem: {
        padding: 'inherit',
        paddingLeft: '20px',
        margin: 'inherit',
        border: 'none',
    },
    msgHighlight: {
        cursor: 'pointer',
    },
    msgType: {
        cursor: 'pointer',
    },
    errorMessageError: {
        color: '#ed0a0a',
    },
});

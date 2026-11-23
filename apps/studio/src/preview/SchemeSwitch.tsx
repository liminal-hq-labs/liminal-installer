// The light and dark choice of a preview whose look has both: the system's colours, or one pinned.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { SCHEMES, type Scheme } from './model';
import styles from './terminal.module.css';

const NAMES: Record<Scheme, Parameters<typeof t>[0]> = {
	system: 'preview.schemeSystem',
	light: 'preview.schemeLight',
	dark: 'preview.schemeDark',
};

export function SchemeSwitch({
	value,
	onChange,
}: {
	value: Scheme;
	onChange: (scheme: Scheme) => void;
}) {
	return (
		<label className={styles.viewSwitch}>
			<span>{t('preview.scheme')}</span>
			<select value={value} onChange={(e) => onChange(e.target.value as Scheme)}>
				{SCHEMES.map((s) => (
					<option key={s} value={s}>
						{t(NAMES[s])}
					</option>
				))}
			</select>
		</label>
	);
}

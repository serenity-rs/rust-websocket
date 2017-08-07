(function() {var implementors = {};
implementors["tokio_core"] = [];
implementors["tokio_tls"] = ["impl&lt;S:&nbsp;<a class=\"trait\" href=\"tokio_core/io/trait.Io.html\" title=\"trait tokio_core::io::Io\">Io</a>&gt; <a class=\"trait\" href=\"tokio_core/io/trait.Io.html\" title=\"trait tokio_core::io::Io\">Io</a> for <a class=\"struct\" href=\"tokio_tls/struct.TlsStream.html\" title=\"struct tokio_tls::TlsStream\">TlsStream</a>&lt;S&gt;",];
implementors["websocket"] = ["impl&lt;S&gt; <a class=\"trait\" href=\"tokio_core/io/trait.Io.html\" title=\"trait tokio_core::io::Io\">Io</a> for <a class=\"struct\" href=\"websocket/client/async/struct.TlsStream.html\" title=\"struct websocket::client::async::TlsStream\">TlsStream</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"tokio_core/io/trait.Io.html\" title=\"trait tokio_core::io::Io\">Io</a>,&nbsp;</span>",];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        
})()

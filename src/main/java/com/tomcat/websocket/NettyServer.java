package com.tomcat.websocket;

import com.tomcat.utils.JwtUtil;
import io.netty.bootstrap.ServerBootstrap;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelOption;
import io.netty.channel.EventLoopGroup;
import io.netty.channel.nio.NioEventLoopGroup;
import io.netty.channel.socket.SocketChannel;
import io.netty.channel.socket.nio.NioServerSocketChannel;
import io.netty.handler.codec.http.HttpObjectAggregator;
import io.netty.handler.codec.http.HttpServerCodec;
import io.netty.handler.codec.http.websocketx.WebSocketServerProtocolHandler;
import io.netty.handler.stream.ChunkedWriteHandler;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.core.userdetails.UserDetailsService;
import org.springframework.stereotype.Component;

import javax.annotation.PostConstruct;

/**
 * NettyServer Netty服务器配置
 */
@Component
@Slf4j
public class NettyServer {

    @Value("${ws.port}")
    private int port;

    @Value("${ws.websocketPath}")
    private String websocketPath;

    @Value("${ws.SO_BACKLOG}")
    private int SO_BACKLOG;

    @Value("${ws.HttpObjectAggregator.maxContentLength}")
    private int maxContentLength;

    @Value("${ws.WebSocketServerProtocolHandler.maxFrameSize}")
    private int maxFrameSize;

    @Autowired
    private JWTDecoder jwtDecoder;

    public NettyServer() {
    }

    public void start() throws Exception {

        EventLoopGroup bossGroup = new NioEventLoopGroup();

        EventLoopGroup group = new NioEventLoopGroup();
        try {
            ServerBootstrap sb = new ServerBootstrap();
            sb.option(ChannelOption.SO_BACKLOG, SO_BACKLOG);
            sb.group(group, bossGroup) // 绑定线程池
                    .channel(NioServerSocketChannel.class) // 指定使用的channel
                    .localAddress(this.port)// 绑定监听端口
                    .childHandler(new ChannelInitializer<SocketChannel>() { // 绑定客户端连接时候触发操作

                        @Override
                        protected void initChannel(SocketChannel ch) throws Exception {
                            log.info("收到新连接");

                            //websocket协议本身是基于http协议的，所以这边也要使用http解编码器
                            ch.pipeline().addLast(new HttpServerCodec());
                            //以块的方式来写的处理器
                            ch.pipeline().addLast(new ChunkedWriteHandler());
                            ch.pipeline().addLast(new HttpObjectAggregator(maxContentLength));
                            // 添加JWT解码器
                            ch.pipeline().addLast(jwtDecoder);
                            ch.pipeline().addLast(new WebSocketServerProtocolHandler(websocketPath, null, true, maxFrameSize));
                            ch.pipeline().addLast(new WebSocketHandler());

                        }
                    });
            ChannelFuture cf = sb.bind().sync(); // 服务器异步创建绑定
            log.info(NettyServer.class + " 启动正在监听： " + cf.channel().localAddress());
            log.info("tomcat websocket start");
            log.info("http://localhost:8000/test/websocket");
            log.info("ws://localhost:"+port+websocketPath);
            cf.channel().closeFuture().sync(); // 关闭服务器通道
        } finally {
            group.shutdownGracefully().sync(); // 释放线程池资源
            bossGroup.shutdownGracefully().sync();
        }
    }

    @PostConstruct
    private void asynStart(){
        log.info("NettyServer asynStart()");
        log.info("${ws.port}: "+ port);
        log.info("${ws.websocketPath}: "+ websocketPath);
        log.info("${ws.SO_BACKLOG}: "+ SO_BACKLOG);
        log.info("maxContentLength: "+ maxContentLength);
        log.info("maxFrameSize: "+ maxFrameSize);

        Thread wsThread = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    NettyServer.this.start();

                } catch (Exception e) {
                    System.out.println("NettyServerError:" + e.getMessage());
                }
            }
        });
        wsThread.setName("websocket");
        wsThread.start();
    }


}

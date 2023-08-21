package com.tomcat.websocket;

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
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import javax.annotation.PostConstruct;

/**
 * NettyServer Netty服务器配置
 */
@Component
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
                            System.out.println("收到新连接");
                            //websocket协议本身是基于http协议的，所以这边也要使用http解编码器
                            ch.pipeline().addLast(new HttpServerCodec());
                            //以块的方式来写的处理器
                            ch.pipeline().addLast(new ChunkedWriteHandler());
                            ch.pipeline().addLast(new HttpObjectAggregator(maxContentLength));
                            ch.pipeline().addLast(new WebSocketServerProtocolHandler(websocketPath, null, true, maxFrameSize));
                            ch.pipeline().addLast(new WebSocketHandler());

                        }
                    });
            ChannelFuture cf = sb.bind().sync(); // 服务器异步创建绑定
            System.out.println(NettyServer.class + " 启动正在监听： " + cf.channel().localAddress());
            System.out.println("tomcat websocket start");
            System.out.println("http://localhost:8000/test/websocket");
            System.out.println("ws://localhost:"+port+websocketPath);
            cf.channel().closeFuture().sync(); // 关闭服务器通道
        } finally {
            group.shutdownGracefully().sync(); // 释放线程池资源
            bossGroup.shutdownGracefully().sync();
        }
    }

    @PostConstruct
    private void asynStart(){
        System.out.println("NettyServer asynStart()");
        System.out.println("${ws.port}: "+ port);
        System.out.println("${ws.websocketPath}: "+ websocketPath);
        System.out.println("${ws.SO_BACKLOG}: "+ SO_BACKLOG);
        System.out.println("maxContentLength: "+ maxContentLength);
        System.out.println("maxFrameSize: "+ maxFrameSize);

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
